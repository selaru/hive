use anyhow::{bail, Context, Result};
use hive_core::{Project, Tool};
use reqwest::Client;
use tracing::{debug, warn};

/// Execute a tool call requested by the LLM and return a string result.
pub async fn run_tool(
    tool:         &Tool,
    arguments:    &serde_json::Value,
    project:      &Project,
    executor_url: &str,
) -> Result<String> {
    let folder = project.folder_path.as_deref().unwrap_or("/tmp/hive");

    match tool.name.as_str() {
        "file_read"     => file_read(arguments, folder),
        "file_write"    => file_write(arguments, folder),
        "file_list"     => file_list(arguments, folder),
        "code_execute"  => code_execute(arguments, folder, executor_url).await,
        "web_search"    => web_search(arguments).await,
        "web_fetch"     => web_fetch(arguments).await,
        "shell_command" => shell_command(arguments, folder, executor_url).await,
        name => {
            if tool.kind == "mcp" {
                let server_url = tool.config["mcp_server_url"]
                    .as_str()
                    .unwrap_or("");
                crate::mcp::call_tool(server_url, name, arguments).await
            } else {
                bail!("Unknown tool: {}", name)
            }
        }
    }
}

// ── Safety helper ─────────────────────────────────────────────────────────────

fn safe_path(relative: &str, root: &str) -> Result<std::path::PathBuf> {
    let root_path = std::fs::canonicalize(root)
        .unwrap_or_else(|_| std::path::PathBuf::from(root));
    let joined = root_path.join(relative.trim_start_matches('/'));

    // Create parent if missing (for write operations)
    let resolved = if joined.exists() {
        std::fs::canonicalize(&joined).context("canonicalize failed")?
    } else {
        // For non-existent paths, resolve parent and re-join filename
        let parent = joined.parent().unwrap_or(&root_path);
        let parent_canonical = if parent.exists() {
            std::fs::canonicalize(parent)?
        } else {
            std::fs::create_dir_all(parent)?;
            std::fs::canonicalize(parent)?
        };
        parent_canonical.join(joined.file_name().unwrap_or_default())
    };

    if !resolved.starts_with(&root_path) {
        bail!("Path traversal rejected: {} is outside project root", relative);
    }
    Ok(resolved)
}

// ── File tools ────────────────────────────────────────────────────────────────

fn file_read(args: &serde_json::Value, root: &str) -> Result<String> {
    let path = args["path"].as_str().context("file_read: missing 'path'")?;
    let full = safe_path(path, root)?;
    debug!("file_read: {:?}", full);
    std::fs::read_to_string(&full)
        .context(format!("file_read: cannot read {:?}", full))
}

fn file_write(args: &serde_json::Value, root: &str) -> Result<String> {
    let path    = args["path"].as_str().context("file_write: missing 'path'")?;
    let content = args["content"].as_str().context("file_write: missing 'content'")?;
    let full    = safe_path(path, root)?;

    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    debug!("file_write: {:?} ({} bytes)", full, content.len());
    std::fs::write(&full, content)
        .context(format!("file_write: cannot write {:?}", full))?;
    Ok(format!("Written {} bytes to {}", content.len(), path))
}

fn file_list(args: &serde_json::Value, root: &str) -> Result<String> {
    let rel_path = args["path"].as_str().unwrap_or(".");
    let full = safe_path(rel_path, root)?;
    debug!("file_list: {:?}", full);

    if !full.exists() {
        return Ok(format!("{} does not exist", rel_path));
    }
    if !full.is_dir() {
        return Ok(format!("{} is a file, not a directory", rel_path));
    }

    let entries: Vec<String> = std::fs::read_dir(&full)?
        .filter_map(|e| e.ok())
        .map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if e.path().is_dir() { format!("{}/", name) } else { name }
        })
        .collect();

    Ok(entries.join("\n"))
}

// ── Code execution ────────────────────────────────────────────────────────────

async fn code_execute(args: &serde_json::Value, root: &str, executor_url: &str) -> Result<String> {
    let language = args["language"].as_str().unwrap_or("python");
    let code     = args["code"].as_str().context("code_execute: missing 'code'")?;
    let timeout  = args["timeout"].as_u64().unwrap_or(30);

    let body = serde_json::json!({
        "language":    language,
        "code":        code,
        "working_dir": format!("{}/src", root),
        "timeout_ms":  timeout * 1000,
    });

    let client = Client::new();
    let resp: serde_json::Value = client
        .post(format!("{}/execute", executor_url))
        .json(&body)
        .send()
        .await
        .context("code_execute: executor unreachable")?
        .json()
        .await
        .context("code_execute: invalid response")?;

    let stdout = resp["stdout"].as_str().unwrap_or("");
    let stderr = resp["stderr"].as_str().unwrap_or("");
    let exit   = resp["exit_code"].as_i64().unwrap_or(-1);

    if exit != 0 {
        Ok(format!("exit_code: {}\nstdout:\n{}\nstderr:\n{}", exit, stdout, stderr))
    } else {
        Ok(if stdout.is_empty() { "(no output)".into() } else { stdout.to_string() })
    }
}

// ── Web tools ─────────────────────────────────────────────────────────────────

async fn web_search(args: &serde_json::Value) -> Result<String> {
    let query = args["query"].as_str().context("web_search: missing 'query'")?;
    let _count = args["count"].as_u64().unwrap_or(5);

    // TODO: integrate Brave Search API or SearXNG
    // For now, return a placeholder
    warn!("web_search not yet implemented (query: {})", query);
    Ok(format!(
        "Web search not yet configured. To enable, set BRAVE_API_KEY in .env.\nQuery was: {}",
        query
    ))
}

async fn web_fetch(args: &serde_json::Value) -> Result<String> {
    let url = args["url"].as_str().context("web_fetch: missing 'url'")?;
    debug!("web_fetch: {}", url);

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let html = client.get(url).send().await?.text().await?;

    // Simple HTML → text conversion (strip tags)
    let text = html
        .lines()
        .map(|l| {
            // Very naive tag stripper — replace with htmd crate for production
            let mut out = String::new();
            let mut in_tag = false;
            for c in l.chars() {
                match c {
                    '<' => in_tag = true,
                    '>' => in_tag = false,
                    _   => if !in_tag { out.push(c); }
                }
            }
            out.trim().to_string()
        })
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    // Truncate to 8000 chars to stay within context limits
    Ok(if text.len() > 8000 {
        format!("{}...[truncated]", &text[..8000])
    } else {
        text
    })
}

// ── Shell command ─────────────────────────────────────────────────────────────

const ALLOWED_COMMANDS: &[&str] = &[
    "cargo", "npm", "npx", "node", "python", "python3", "pip", "pip3",
    "ls", "cat", "grep", "find", "mkdir", "cp", "mv", "echo", "pwd",
    "wc", "sort", "uniq", "head", "tail", "diff",
];

async fn shell_command(args: &serde_json::Value, root: &str, executor_url: &str) -> Result<String> {
    let command = args["command"].as_str().context("shell_command: missing 'command'")?;

    let first_word = command.split_whitespace().next().unwrap_or("");
    if !ALLOWED_COMMANDS.contains(&first_word) {
        bail!(
            "shell_command: '{}' is not in the allowlist. Allowed: {}",
            first_word,
            ALLOWED_COMMANDS.join(", ")
        );
    }

    let body = serde_json::json!({
        "language":    "bash",
        "code":        command,
        "working_dir": root,
        "timeout_ms":  30000,
    });

    let client = Client::new();
    let resp: serde_json::Value = client
        .post(format!("{}/execute", executor_url))
        .json(&body)
        .send()
        .await
        .context("shell_command: executor unreachable")?
        .json()
        .await?;

    let stdout = resp["stdout"].as_str().unwrap_or("");
    let stderr = resp["stderr"].as_str().unwrap_or("");
    let exit   = resp["exit_code"].as_i64().unwrap_or(-1);

    Ok(format!(
        "exit_code: {}\n{}{}",
        exit,
        stdout,
        if stderr.is_empty() { String::new() } else { format!("\nstderr:\n{}", stderr) }
    ))
}
