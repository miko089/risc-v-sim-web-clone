use axum::{
    Router,
    extract::Multipart,
    http::StatusCode,
    response::Response,
    routing::{get, post},
};
use std::process::Command;
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::timeout;

async fn health_handler() -> &'static str {
    "Ok"
}

fn compile_s_to_elf(
    s_content: &[u8],
    as_binary: &str,
    ld_binary: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let dir = tempdir()?;
    let s_path = dir.path().join("input.s");
    let o_path = dir.path().join("output.o");
    let elf_path = dir.path().join("output.elf");

    std::fs::write(&s_path, s_content)?;

    let as_output = Command::new(as_binary)
        .arg(&s_path)
        .arg("-o")
        .arg(&o_path)
        .output()?;

    if !as_output.status.success() {
        return Err("assembler failed".into());
    }

    let ld_output = Command::new(ld_binary)
        .arg(&o_path)
        .arg("-Ttext=0x80000000")
        .arg("-o")
        .arg(&elf_path)
        .output()?;

    if !ld_output.status.success() {
        return Err("linker failed".into());
    }

    let elf_content = std::fs::read(&elf_path)?;
    Ok(elf_content)
}

fn run_simulator(
    elf_content: &[u8],
    ticks: u32,
    simulator_binary: &str,
) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let dir = tempdir()?;
    let elf_path = dir.path().join("sim_input.elf");
    std::fs::write(&elf_path, elf_content)?;

    let output = Command::new(simulator_binary)
        .arg("--ticks")
        .arg(ticks.to_string())
        .arg("--path")
        .arg(&elf_path)
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;

    Ok((stdout, stderr))
}

async fn submit_handler(mut multipart: Multipart) -> Result<Response, StatusCode> {
    let as_binary = std::env::var("AS_BINARY").unwrap_or_else(|_| "riscv64-elf-as".to_string());
    let ld_binary = std::env::var("LD_BINARY").unwrap_or_else(|_| "riscv64-elf-ld".to_string());
    let simulator_binary =
        std::env::var("SIMULATOR_BINARY").unwrap_or_else(|_| "simulator".to_string());

    let ticks_field = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let ticks_str = ticks_field
        .ok_or(StatusCode::BAD_REQUEST)?
        .text()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let ticks: u32 = ticks_str.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    let file_field = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_content = file_field
        .ok_or(StatusCode::BAD_REQUEST)?
        .bytes()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let compile_result = timeout(
        Duration::from_secs(5),
        tokio::task::spawn_blocking(move || {
            compile_s_to_elf(&file_content, &as_binary, &ld_binary)
        }),
    )
    .await
    .map_err(|_| StatusCode::REQUEST_TIMEOUT)?;

    let elf_content = compile_result
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let sim_result = timeout(
        Duration::from_secs(10),
        tokio::task::spawn_blocking(move || run_simulator(&elf_content, ticks, &simulator_binary)),
    )
    .await
    .map_err(|_| StatusCode::REQUEST_TIMEOUT)?;

    let (stdout, stderr) = sim_result
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !stderr.is_empty() {
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "text/plain")
            .body(stderr.into())
            .unwrap());
    }

    match serde_json::from_str::<serde_json::Value>(&stdout) {
        Ok(_) => Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(stdout.into())
            .unwrap()),
        Err(_) => {
            eprintln!("Invalid JSON output: {}", stdout);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/submit", post(submit_handler));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
