use axum::{routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{net::TcpListener, process::Command, time::timeout};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Deserialize)]
struct ExecuteRequest {
    language:    String,
    code:        String,
    working_dir: String,
    timeout_ms:  Option<u64>,
}

#[derive(Debug, Serialize)]
struct ExecuteResponse {
    stdout:      String,
    stderr:      String,
    exit_code:   i32,
    duration_ms: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "hive_executor=debug,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new().route("/execute", post(execute_handler));

    let listener = TcpListener::bind("0.0.0.0:3001").await?;
    tracing::info!("Hive Executor listening on 0.0.0.0:3001");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn execute_handler(
    Json(req): Json<ExecuteRequest>,
) -> Json<ExecuteResponse> {
    let start = std::time::Instant::now();
    let timeout_ms = req.timeout_ms.unwrap_or(30_000);

    let result = run_in_docker(&req, timeout_ms).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok((stdout, stderr, exit_code)) => Json(ExecuteResponse {
            stdout, stderr, exit_code, duration_ms,
        }),
        Err(e) => Json(ExecuteResponse {
            stdout:      String::new(),
            stderr:      format!("Executor error: {}", e),
            exit_code:   -1,
            duration_ms,
        }),
    }
}

async fn run_in_docker(
    req: &ExecuteRequest,
    timeout_ms: u64,
) -> anyhow::Result<(String, String, i32)> {
    let (image, ext) = match req.language.as_str() {
        "python"     => ("python:3.12-slim",    "py"),
        "javascript" => ("node:22-alpine",       "js"),
        "bash"       => ("debian:bookworm-slim", "sh"),
        other        => anyhow::bail!("Unsupported language: {}", other),
    };

    // Write code to a temp file within the working dir
    let code_file = format!("/tmp/hive_exec_{}.{}", uuid::Uuid::new_v4(), ext);
    tokio::fs::write(&code_file, &req.code).await?;

    // Ensure working dir exists
    let working_dir = PathBuf::from(&req.working_dir);
    tokio::fs::create_dir_all(&working_dir).await?;

    let run_cmd = match req.language.as_str() {
        "python"     => format!("python /code"),
        "javascript" => format!("node /code"),
        "bash"       => format!("bash /code"),
        _            => format!("/code"),
    };

    let mut cmd = Command::new("docker");
    cmd.args([
        "run", "--rm",
        "--network", "none",           // no network access for code execution
        "--memory", "512m",
        "--cpus", "1",
        "-v", &format!("{}:/workspace", working_dir.display()),
        "-v", &format!("{}:/code", code_file),
        "-w", "/workspace",
        image,
        "sh", "-c", &run_cmd,
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    let child = cmd.spawn()?;

    let result = timeout(Duration::from_millis(timeout_ms), child.wait_with_output()).await;

    // Clean up temp file
    let _ = tokio::fs::remove_file(&code_file).await;

    match result {
        Err(_) => {
            // Timeout — try to kill
            Ok((
                String::new(),
                format!("Execution timed out after {}ms", timeout_ms),
                -1,
            ))
        }
        Ok(Err(e)) => Err(anyhow::anyhow!("Process error: {}", e)),
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);
            Ok((stdout, stderr, exit_code))
        }
    }
}
