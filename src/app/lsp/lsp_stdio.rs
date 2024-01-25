use super::lsp_client::LspClient;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use lsp_types::*;
use serde::Serialize;
use serde_json::{from_value, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

use std::process::Stdio;
use tokio::io::{BufReader, BufWriter};
use tokio::process::{Child, Command};

use tokio::io::AsyncReadExt;
pub struct LspClientStdio {
  sequence: u64,
  child: Child,
}

#[async_trait]
impl LspClient for LspClientStdio {
  async fn initialize(&mut self, initialization_params: InitializeParams) -> Result<InitializeResult> {
    let request_id = self.next_id();
    // let result = self.send_request::<InitializeParams>("initialize", None, request_id).await;
    let result = self.send_request("initialize", Some(initialization_params), request_id).await;
    match result {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "initialize" to context of error
        Err(anyhow!("failed to initialize LSP client: {}", err))
      },
    }
  }

  async fn shutdown(&mut self) -> Result<()> {
    let request_id = self.next_id();
    self.send_request::<Value>("shutdown", None::<Value>, request_id).await?;
    Ok(())
  }

  async fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Result<()> {
    self.send_notification("textDocument/didOpen", params).await
  }
}

impl LspClientStdio {
  fn next_id(&self) -> u64 {
    self.sequence.wrapping_add(1)
  }

  async fn send_request<T: Serialize>(&mut self, method: &str, params: Option<T>, id: u64) -> Result<Value> {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    });
    println!("request: {:#?}", request);
    let request_str = serde_json::to_string(&request)?;
    let irequest_str =
      r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"capabilities":{},"processId":null,"rootUri":null}}"#;

    let arequest_str = "asdkasldasd";
    println!("request_str: {}", request_str);

    let stdin = self.child.stdin.take().expect("unable to access stdin");
    tokio::spawn(async move {
      let mut writer = BufWriter::new(stdin);
      writer.write_all(request_str.as_bytes()).await.unwrap();
      writer.write_all(b"\n").await.unwrap();
      writer.flush().await.unwrap();
    });
    // .await
    // .unwrap();
    // let _ = self.child.wait().await;
    self.read_response(id).await
  }

  async fn send_notification<T: Serialize>(&mut self, method: &str, params: T) -> Result<()> {
    let notification = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params
    });

    let notification_str = serde_json::to_string(&notification)?;
    let mut stdin = self.child.stdin.take().expect("unable to access stdin");
    tokio::spawn(async move {
      stdin.write_all(notification_str.as_bytes()).await.unwrap();
      stdin.write_all(b"\n").await.unwrap();
      stdin.flush().await.unwrap();
      drop(stdin);
    });
    // let _ = self.child.wait().await;
    Ok(())
  }
  async fn read_response(&mut self, expected_id: u64) -> Result<Value> {
    let mut response_str = String::new();
    let stdout = self.child.stdout.take().expect("unable to access stdout");
    // let _ = self.child.wait().await;
    let mut reader = BufReader::new(stdout);
    if reader.read_to_string(&mut response_str).await? == 0 {
      // EOF found on stdout, so check stderr for any error messages
      let mut stderr_output = String::new();
      let stderr = self.child.stderr.take().expect("unable to access stderr");
      let mut stderr_reader = BufReader::new(stderr);
      stderr_reader.read_to_string(&mut stderr_output).await?;

      if !stderr_output.is_empty() {
        eprintln!("LSP stderr: {}", stderr_output);
      }

      return Err(anyhow!("LSP err: {}", stderr_output));
    }
    println!("response_str: {}", response_str);
    let response: Value = serde_json::from_str(&response_str)?;
    let response_id =
      response.get("id").and_then(Value::as_u64).ok_or_else(|| anyhow!("Missing or invalid 'id' field in response"))?;

    if response_id != expected_id {
      return Err(anyhow!("Mismatched response id: expected {}, got {}", expected_id, response_id));
    }

    if let Some(error) = response.get("error") {
      Err(anyhow!("Error in response: {:?}", error))
    } else {
      response.get("result").cloned().ok_or_else(|| anyhow!("Missing 'result' field in response"))
    }
  }
}

impl LspClientStdio {
  pub async fn create() -> anyhow::Result<Self> {
    // test command that runs rust-analyzer --version and prints the output

    let output = Command::new("which")
    .arg("rust-analyzer")
             // .current_dir("/bin")
             .output().await.unwrap();
    println!("rust-analyzer --version: {}", String::from_utf8_lossy(&output.stdout));
    let child =
      // Command::new("rust-analyzer").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    Command::new("rust-analyzer").arg("-v").arg("-v").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("rust-analyzer failed to start");

    // Command::new("env").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

    // let stdout = child.stdout.take().expect("Child process should have a stdout");
    // let stdin = child.stdin.take().expect("Child process should have a stdin.");
    // let stderr = child.stderr.take().expect("Child process should have a stderr.");
    //
    // let writer = BufWriter::new(stdin);
    // let reader = BufReader::new(stdout);
    // let stderr_reader = BufReader::new(stderr);

    // Ok(Self { reader, writer, stderr_reader, child, sequence: 0 })
    Ok(Self { child, sequence: 0 })
  }

  // // Asynchronous function to constantly read from stderr
  // async fn read_stderr(mut stderr: ChildStderr) {
  //   let mut buffer = String::new();
  //   while let Ok(n) = stderr.read_to_string(&mut buffer).await {
  //     if n == 0 {
  //       break; // EOF reached
  //     }
  //
  //     if !buffer.is_empty() {
  //       eprintln!("rust-analyzer stderr: {}", buffer);
  //       buffer.clear(); // Clear buffer to avoid printing the same message repeatedly.
  //     }
  //   }
  // }
}
