use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use futures::StreamExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name:        String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// ── SSE text helpers ─────────────────────────────────────────────────────────

fn extract_sse_endpoint(buffer: &str, server_url: &str) -> Option<String> {
    let pos = buffer.find("event: endpoint")?;
    let remainder = &buffer[pos..];
    let data_start = remainder.find("data: ")? + 6;
    let newline = remainder[data_start..].find('\n')?;
    let endpoint_url = remainder[data_start..(data_start + newline)].trim().to_string();

    let resolved = if endpoint_url.starts_with("http://") || endpoint_url.starts_with("https://") {
        endpoint_url
    } else {
        reqwest::Url::parse(server_url)
            .and_then(|base| base.join(&endpoint_url))
            .map(|u| u.to_string())
            .unwrap_or(endpoint_url)
    };
    Some(resolved)
}

/// Returns `(json_payload, consumed_up_to)` for the first `event: message` in the buffer.
fn extract_sse_message(buffer: &str) -> Option<(String, usize)> {
    let pos = buffer.find("event: message")?;
    let remainder = &buffer[pos..];
    let data_start = remainder.find("data: ")? + 6;
    let newline = remainder[data_start..].find('\n')?;
    let payload = remainder[data_start..(data_start + newline)].trim().to_string();
    Some((payload, pos + data_start + newline + 1))
}

// ── Core: full MCP session lifecycle ─────────────────────────────────────────

/// Runs a complete MCP session against `server_url`:
///
///  1. GET SSE endpoint → wait for `event: endpoint` to get the session POST URL
///  2. POST `initialize`  → wait for response
///  3. POST `initialized` notification (no response expected)
///  4. POST `method`      → wait for response
///
/// The SSE connection is held open across all steps so the server does not
/// invalidate the session before we finish.
///
/// Response handling tries the POST body first (some implementations return the
/// JSON-RPC result there); if the body is empty / non-JSON-RPC it falls back to
/// reading the next `event: message` from the SSE stream.
async fn mcp_call(
    server_url: &str,
    method:     &str,
    params:     serde_json::Value,
) -> Result<serde_json::Value> {

    // ── helpers defined as closures so they share `stream` / `buf` ────────────

    // Send a request and return the parsed JSON-RPC response.
    // Caller must pass `stream` and `buf` so the SSE connection stays in scope.
    macro_rules! rpc {
        ($client:expr, $url:expr, $req:expr, $stream:expr, $buf:expr) => {{
            let resp = $client
                .post($url)
                .json(&$req)
                .send()
                .await
                .context(format!("Failed to POST to MCP endpoint: {}", $url))?;

            let status = resp.status();
            let body = resp.text().await.context("Failed to read POST body")?;

            // Strategy A: JSON-RPC result in POST body
            if status.is_success() && !body.trim().is_empty() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if json.get("jsonrpc").is_some()
                        || json.get("result").is_some()
                        || json.get("error").is_some()
                    {
                        json
                    } else {
                        // Not a JSON-RPC response – fall through to SSE
                        read_sse_message($stream, $buf).await?
                    }
                } else {
                    read_sse_message($stream, $buf).await?
                }
            } else if !status.is_success() {
                anyhow::bail!("MCP server returned error status {}: {}", status, body);
            } else {
                // Strategy B: 202 Accepted → response via SSE
                read_sse_message($stream, $buf).await?
            }
        }};
    }

    // ── 1. Open SSE connection ────────────────────────────────────────────────
    let sse_client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .context("Failed to build SSE client")?;

    let sse_resp = sse_client
        .get(server_url)
        .send()
        .await
        .context(format!("Failed to connect to MCP SSE endpoint: {}", server_url))?;

    let mut stream = sse_resp.bytes_stream();
    let mut buf = String::new();

    // ── 2. Wait for `event: endpoint` ────────────────────────────────────────
    let post_url = loop {
        match stream.next().await {
            Some(Ok(chunk)) => {
                if let Ok(text) = std::str::from_utf8(&chunk) {
                    buf.push_str(text);
                }
                if let Some(url) = extract_sse_endpoint(&buf, server_url) {
                    buf.clear();
                    break url;
                }
                if buf.len() > 4096 {
                    anyhow::bail!(
                        "MCP SSE server at {} did not send 'event: endpoint' in the first 4 KiB",
                        server_url
                    );
                }
            }
            Some(Err(e)) => anyhow::bail!("SSE stream error: {}", e),
            None => anyhow::bail!("SSE stream closed before 'event: endpoint'"),
        }
    };

    let post_client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("Failed to build POST client")?;

    // ── 3. initialize ─────────────────────────────────────────────────────────
    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "hive", "version": "0.1.0" }
        }
    });
    let init_resp = rpc!(&post_client, &post_url, init_req, &mut stream, &mut buf);
    if let Some(err) = init_resp.get("error") {
        anyhow::bail!("MCP initialize failed: {}", err);
    }

    // ── 4. initialized notification (no response) ─────────────────────────────
    post_client
        .post(&post_url)
        .json(&serde_json::json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }))
        .send()
        .await
        .context("Failed to send 'initialized' notification")?;

    // ── 5. actual method call ─────────────────────────────────────────────────
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": method,
        "params": params
    });
    let result = rpc!(&post_client, &post_url, request, &mut stream, &mut buf);

    drop(stream);
    Ok(result)
}

/// Reads the next `event: message` payload from an open SSE byte stream.
async fn read_sse_message<S>(stream: &mut S, buf: &mut String) -> Result<serde_json::Value>
where
    S: futures::Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin,
{
    buf.clear();
    loop {
        match stream.next().await {
            Some(Ok(chunk)) => {
                if let Ok(text) = std::str::from_utf8(&chunk) {
                    buf.push_str(text);
                }
                if let Some((msg, consumed)) = extract_sse_message(buf) {
                    *buf = buf[consumed..].to_string();
                    return serde_json::from_str(&msg)
                        .context(format!("Invalid JSON in SSE message: {}", msg));
                }
                if buf.len() > 65_536 {
                    anyhow::bail!("SSE response message exceeded 64 KiB");
                }
            }
            Some(Err(e)) => anyhow::bail!("SSE stream error: {}", e),
            None => anyhow::bail!("SSE stream closed before a response message was received"),
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Discover all tools from an MCP server via SSE transport.
pub async fn list_tools(server_url: &str) -> Result<Vec<McpTool>> {
    let resp = mcp_call(server_url, "tools/list", serde_json::json!({})).await?;

    if let Some(err) = resp.get("error") {
        anyhow::bail!("MCP error from tools/list: {}", err);
    }

    let tools = resp["result"]["tools"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    tools
        .into_iter()
        .map(|t| {
            Ok(McpTool {
                name:         t["name"].as_str().unwrap_or("").to_string(),
                description:  t["description"].as_str().unwrap_or("").to_string(),
                input_schema: t["inputSchema"].clone(),
            })
        })
        .collect()
}

/// Call a specific tool on an MCP server.
pub async fn call_tool(
    server_url: &str,
    tool_name:  &str,
    arguments:  &serde_json::Value,
) -> Result<String> {
    let resp = mcp_call(
        server_url,
        "tools/call",
        serde_json::json!({ "name": tool_name, "arguments": arguments }),
    )
    .await?;

    if let Some(err) = resp.get("error") {
        anyhow::bail!("MCP error: {}", err);
    }

    let content = &resp["result"]["content"];
    if let Some(arr) = content.as_array() {
        let texts: Vec<&str> = arr
            .iter()
            .filter_map(|c| {
                if c["type"].as_str() == Some("text") {
                    c["text"].as_str()
                } else {
                    None
                }
            })
            .collect();
        Ok(texts.join("\n"))
    } else {
        Ok(resp["result"].to_string())
    }
}
