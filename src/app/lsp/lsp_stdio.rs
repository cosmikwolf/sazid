use std::{io, process::Stdio};

use anyhow::{anyhow, Result};
use lsp_types::*;
use serde::Serialize;
use serde_json::{from_value, json, Value};
use tokio::{
  io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
  process::{ChildStderr, ChildStdin, ChildStdout, Command},
};

use super::lsp_client::LspClient;
pub struct LspClientStdio {
  capabilites: ClientCapabilities,
  sequence: u64,
  stdout_buf: BufReader<ChildStdout>,
  stdin_buf: BufWriter<ChildStdin>,
  stderr_buf: BufReader<ChildStderr>,
}

impl LspClient for LspClientStdio {
  fn add_client_capabilities(&mut self) {
    let new_capabilites = ClientCapabilities {
      workspace: Some(WorkspaceClientCapabilities {
        workspace_folders: Some(true),
        workspace_edit: Some(WorkspaceEditClientCapabilities {
          document_changes: Some(true),
          resource_operations: Some(vec![
            ResourceOperationKind::Create,
            ResourceOperationKind::Delete,
            ResourceOperationKind::Rename,
          ]),
          failure_handling: Some(FailureHandlingKind::TextOnlyTransactional),
          normalizes_line_endings: Some(true),
          ..Default::default()
        }),
        ..Default::default()
      }),
      text_document: Some(TextDocumentClientCapabilities {
        synchronization: Some(TextDocumentSyncClientCapabilities {
          dynamic_registration: Some(true),
          will_save: Some(true),
          will_save_wait_until: Some(true),
          did_save: Some(true),
        }),
        ..Default::default()
      }),
      ..Default::default()
    };
    self.update_capabilities(new_capabilites).unwrap();
  }
  fn update_capabilities(&mut self, new_capabilities: ClientCapabilities) -> anyhow::Result<()> {
    fn deep_merge(base: &mut Value, other: &Value) {
      match (base, other) {
        // If both values are objects, recursively merge
        (Value::Object(ref mut base_map), Value::Object(other_map)) => {
          for (key, other_value) in other_map {
            deep_merge(base_map.entry(key).or_insert(Value::Null), other_value);
          }
        },
        // Overwrite the base value with the other value
        (base, other) => *base = other.clone(),
      }
    }

    let mut capabilities = json!(self.capabilites);
    let new_capabilities = json!(new_capabilities);
    self.capabilites = from_value(deep_merge(&mut capabilities, &new_capabilities).into()).unwrap();
    Ok(())
  }
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
    let request_str = serde_json::to_string(&request)?;
    let request_str = request_str.trim();
    println!("> {}", request_str);
    let request_str = format!("Content-Length: {}\r\n\r\n{}", request_str.len() as usize, request_str);

    self.stdin_buf.write_all(request_str.as_bytes()).await.unwrap();
    match self.stdin_buf.flush().await {
      Ok(_) => self.read_response(id).await,
      Err(e) => {
        self.read_error().await?;
        Err(anyhow!("failed to flush stdin: {}", e))
      },
    }
  }

  async fn send_notification<T: Serialize>(&mut self, method: &str, params: T) -> Result<()> {
    let notification = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params
    });

    let notification_str = serde_json::to_string(&notification)?;
    let notification_str = notification_str.trim();

    let notification_str =
      format!("Content-Length: {}\r\n\r\n{}", notification_str.len().to_string().trim(), notification_str);

    let notification_bytes = notification_str.as_bytes();

    self.stdin_buf.write_all(notification_bytes).await.unwrap();
    match self.stdin_buf.flush().await {
      Ok(_) => Ok(()),
      Err(e) => {
        self.read_error().await?;
        Err(anyhow!("failed to flush stdin: {}", e))
      },
    }
  }

  async fn read_error(&mut self) -> Result<()> {
    let mut stderr_output = String::new();
    self.stderr_buf.read_to_string(&mut stderr_output).await?;
    if !stderr_output.is_empty() {
      eprintln!("LSP stderr: {}", stderr_output);
    }
    Ok(())
  }

  async fn read_response(&mut self, expected_id: u64) -> Result<Value> {
    fn invalid_data(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
      io::Error::new(io::ErrorKind::InvalidData, error)
    }
    macro_rules! invalid_data {
        ($($tt:tt)*) => (invalid_data(format!($($tt)*)))
    }

    let mut size = None;
    let mut buf = String::new();
    loop {
      buf.clear();
      if self.stdout_buf.read_line(&mut buf).await.unwrap() == 0 {
        self.read_error().await?;
        return Err(invalid_data!("unexpected EOF").into());
      }
      if !buf.ends_with("\r\n") {
        return Err(invalid_data!("malformed header: {:?}", buf).into());
      }
      let buf = &buf[..buf.len() - 2];
      if buf.is_empty() {
        break;
      }
      let mut parts = buf.splitn(2, ": ");
      let header_name = parts.next().unwrap();
      let header_value = parts.next().ok_or_else(|| invalid_data(format!("malformed header: {:?}", buf)))?;
      if header_name.eq_ignore_ascii_case("Content-Length") {
        size = Some(header_value.parse::<usize>().map_err(invalid_data)?);
      }
    }
    let size: usize = size.ok_or_else(|| invalid_data("no Content-Length".to_string()))?;
    let mut buf = buf.into_bytes();
    buf.resize(size, 0);
    self.stdout_buf.read_exact(&mut buf).await?;
    let buf = String::from_utf8(buf).map_err(invalid_data)?;
    log::debug!("< {}", buf);
    println!("< {}", buf);
    let response: Value = serde_json::from_str(&buf)?;

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

  async fn create() -> anyhow::Result<Self> {
    // test command that runs rust-analyzer --version and prints the output

    // let output = Command::new("which")
    // .arg("rust-analyzer")
    // let output = Command::new("rust-analyzer")
    // .arg("--version")
    //          // .current_dir("/bin")
    //          .output().await.unwrap();
    // println!("rust-analyzer --version: {}", String::from_utf8_lossy(&output.stdout));
    let mut child =
      // Command::new("rust-analyzer").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    Command::new("rust-analyzer").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("rust-analyzer failed to start");

    // Command::new("env").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

    let stdout = child.stdout.take().expect("Child process should have a stdout");
    let stdin = child.stdin.take().expect("Child process should have a stdin.");
    let stderr = child.stderr.take().expect("Child process should have a stderr.");

    let stdin_buf = BufWriter::new(stdin);
    let stdout_buf = BufReader::new(stdout);
    let stderr_buf = BufReader::new(stderr);

    // Ok(Self { reader, writer, stderr_reader, child, sequence: 0 })
    Ok(Self { stdin_buf, stdout_buf, stderr_buf, sequence: 0 })
  }
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

  async fn initialized(&mut self) -> Result<()> {
    let params = InitializedParams {};
    self.send_notification("initialized", params).await?;
    Ok(())
  }

  async fn shutdown(&mut self) -> Result<()> {
    let request_id = self.next_id();
    self.send_request::<Value>("shutdown", None::<Value>, request_id).await?;
    Ok(())
  }

  async fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Result<()> {
    match self.send_notification("textDocument/didOpen", params).await {
      Ok(_) => Ok(()),
      Err(err) => {
        // add "textDocument/didOpen" to context of error
        Err(anyhow!("failed to send textDocument/didOpen notification: {}", err))
      },
    }
  }

  async fn did_change(&mut self, params: DidChangeTextDocumentParams) -> anyhow::Result<()> {
    match self.send_notification("textDocument/didChange", params).await {
      Ok(_) => Ok(()),
      Err(err) => {
        // add "textDocument/didChange" to context of error
        Err(anyhow!("failed to send textDocument/didChange notification: {}", err))
      },
    }
  }

  async fn did_save(&mut self, params: DidSaveTextDocumentParams) -> anyhow::Result<()> {
    match self.send_notification("textDocument/didSave", params).await {
      Ok(_) => Ok(()),
      Err(err) => {
        // add "textDocument/didSave" to context of error
        Err(anyhow!("failed to send textDocument/didSave notification: {}", err))
      },
    }
  }

  async fn did_close(&mut self, params: DidCloseTextDocumentParams) -> anyhow::Result<()> {
    match self.send_notification("textDocument/didClose", params).await {
      Ok(_) => Ok(()),
      Err(err) => {
        // add "textDocument/didClose" to context of error
        Err(anyhow!("failed to send textDocument/didClose notification: {}", err))
      },
    }
  }
}
