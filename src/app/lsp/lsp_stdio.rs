use std::sync::Arc;
use std::time::Duration;
use std::{io, process::Stdio};

use anyhow::{anyhow, Result};
use backoff::future::retry;
use backoff::ExponentialBackoff;
use futures_util::Future;
use lsp_types::*;
use serde::Serialize;
use serde_json::{from_value, json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{mpsc, Mutex, MutexGuard};
use tokio::{
  io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
  process::{ChildStderr, ChildStdin, ChildStdout, Command},
};

// other state fields

use super::lsp_client::{LspClient, State};

pub struct StdioBuffers {
  stdout_buf: BufReader<ChildStdout>,
  stdin_buf: BufWriter<ChildStdin>,
  stderr_buf: BufReader<ChildStderr>,
}

impl StdioBuffers {
  async fn check_work_done_progress(
    &mut self,
    id: u64,
    token: ProgressToken,
  ) -> anyhow::Result<Option<WorkDoneProgressCreateParams>> {
    match self.send_request("$/progress", Some(WorkDoneProgressCreateParams { token }), id).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "window/workDoneProgress/create" to context of error
        Err(anyhow!("failed to send window/workDoneProgress/create request: {}", err))
      },
    }
  }

  pub async fn send_request<T: Serialize>(&mut self, method: &str, params: Option<T>, id: u64) -> Result<Value> {
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
}

pub struct LspClientStdio {
  pub capabilities: ClientCapabilities,
  state: Arc<Mutex<State>>,
  sequence: u64,
  work_done_tokens: Vec<ProgressToken>,
  stdio: Arc<Mutex<StdioBuffers>>,
}

impl LspClient for LspClientStdio {
  async fn get_stdio_handle<F, T>(&self, f: F) -> T
  where
    F: FnOnce(&mut StdioBuffers) -> T + Send,
    T: Send,
    F: Send,
  {
    let mut stdio = self.stdio.lock().await;
    f(&mut *stdio)
  }

  async fn send_request<T: Serialize>(&mut self, method: &str, params: Option<T>, id: u64) -> Result<Value> {
    self.stdio.lock().await.send_request(method, params, id).await
  }
  async fn send_notification<T: Serialize>(&mut self, method: &str, params: T) -> Result<()> {
    self.stdio.lock().await.send_notification(method, params).await
  }

  async fn create() -> anyhow::Result<Self> {
    let state = Arc::new(Mutex::new(State { sequence: AtomicU64::new(0) }));
    let mut child = Command::new("rust-analyzer")
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .expect("rust-analyzer failed to start");

    let stdout = child.stdout.take().expect("Child process should have a stdout");
    let stdin = child.stdin.take().expect("Child process should have a stdin.");
    let stderr = child.stderr.take().expect("Child process should have a stderr.");

    let stdio = Arc::new(Mutex::new(StdioBuffers {
      stdout_buf: BufReader::new(stdout),
      stdin_buf: BufWriter::new(stdin),
      stderr_buf: BufReader::new(stderr),
    }));

    let capabilities = ClientCapabilities {
      workspace: Some(WorkspaceClientCapabilities {
        symbol: Some(WorkspaceSymbolClientCapabilities {
          dynamic_registration: Some(true),
          symbol_kind: Some(SymbolKindCapability {
            value_set: Some(vec![
              SymbolKind::FILE,
              SymbolKind::MODULE,
              SymbolKind::NAMESPACE,
              SymbolKind::PACKAGE,
              SymbolKind::CLASS,
              SymbolKind::METHOD,
              SymbolKind::PROPERTY,
              SymbolKind::FIELD,
              SymbolKind::CONSTRUCTOR,
              SymbolKind::ENUM,
              SymbolKind::INTERFACE,
              SymbolKind::FUNCTION,
              SymbolKind::VARIABLE,
              SymbolKind::CONSTANT,
              SymbolKind::STRING,
              SymbolKind::NUMBER,
              SymbolKind::BOOLEAN,
              SymbolKind::ARRAY,
              SymbolKind::OBJECT,
              SymbolKind::KEY,
              SymbolKind::NULL,
              SymbolKind::ENUM_MEMBER,
              SymbolKind::STRUCT,
              SymbolKind::EVENT,
              SymbolKind::OPERATOR,
              SymbolKind::TYPE_PARAMETER,
            ]),
          }),
          tag_support: None,
          resolve_support: Some(WorkspaceSymbolResolveSupportCapability { properties: vec![] }),
        }),
        workspace_folders: None,
        file_operations: None,
        ..Default::default()
      }),
      ..Default::default()
    };

    // Ok(Self { reader, writer, stderr_reader, child, sequence: 0 })
    Ok(Self { capabilities, stdio, state, sequence: 0 })
  }

  fn next_id(&self) -> u64 {
    self.sequence.wrapping_add(1)
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

  async fn handle_server_messages(mut receiver: mpsc::Receiver<String>, state: Arc<Mutex<State>>) {
    while let Some(message) = receiver.recv().await {
      let state = state.lock().await;
      // process message and update state
      state.sequence.fetch_add(1, Ordering::SeqCst);
      println!("Processed a message: {}", message);
      // Drop the lock automatically at the end of this block
    }
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

    let mut capabilities = json!(self.capabilities);
    let new_capabilities = json!(new_capabilities);
    self.capabilities = from_value({
      deep_merge(&mut capabilities, &new_capabilities);
      ().into()
    })
    .unwrap();
    Ok(())
  }

  async fn workspace_symbol_query(&mut self, query: &str) -> anyhow::Result<Option<Vec<SymbolInformation>>> {
    let work_done_token = rand::random::<i32>();
    let partial_result_params = rand::random::<i32>();

    println!("progress_params: {}", partial_result_params);
    println!("WDT: {}", work_done_token);
    fn check_work_done_token(data: &Value, expected_token: i32) -> Result<bool> {
      // Navigate through the JSON structure to find the `workDoneToken` key
      println!("Data: {:#?}", data);

      match data[0].get("params") {
        Some(params) => {
          println!("Params: {}", params);
          match params.get("workDoneToken") {
            Some(work_done_token) => {
              println!("WDT: {}", work_done_token);

              if work_done_token == expected_token {
                Ok(true)
              } else {
                Err(anyhow!("workDoneToken does not match"))
              }
            },
            None => Ok(false),
          }
        },
        None => Ok(false),
      }
    }

    let eb = ExponentialBackoff { max_elapsed_time: Some(Duration::from_millis(10000)), ..Default::default() };
    backoff::future::retry(eb, move || {
      let stdio = self.stdio.clone();
      let params = WorkspaceSymbolParams {
        query: query.to_string(),
        ..Default::default() // work_done_progress_params: WorkDoneProgressParams {
                             //   work_done_token: Some(NumberOrString::Number(work_done_token)),
                             // },
                             // partial_result_params: PartialResultParams {
                             //   partial_result_token: Some(NumberOrString::Number(partial_result_params)),
                             // },
      };
      let id = self.next_id();
      async move {
        let mut stdio = stdio.lock().await;
        match stdio.send_request("workspace/symbol", Some(params), id).await {
          Ok(result) => match check_work_done_token(&result, work_done_token) {
            Ok(true) => Ok(from_value(result).map_err(Into::into)),
            Ok(false) => {
              Err(backoff::Error::transient(anyhow!("workspace/symbol request did not return workDoneToken")))
            },
            Err(e) => Err(backoff::Error::permanent(anyhow!(e))),
          },
          Err(err) => {
            // add "workspace/symbol" to context of error
            Err(backoff::Error::permanent(anyhow!("failed to send workspace/symbol request: {}", err)))
          },
        }
      }
    })
    .await?
  }

  async fn start_work_progress_loop(&mut self) -> anyhow::Result<()> {
    let work_done_progress_begin = WorkDoneProgressBegin {
      title: "Processing".to_string(),
      cancellable: Some(true),
      message: None,
      percentage: None,
    };
  }
}
