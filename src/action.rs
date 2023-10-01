use async_openai::types::CreateChatCompletionResponse;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
  Quit,
  Tick,
  Render,
  Resume,
  Suspend,
  Resize(u16, u16),
  ToggleShowHelp,
  ScheduleIncrement,
  ScheduleDecrement,
  Increment(usize),
  Decrement(usize),
  CompleteInput(String),
  EnterNormal,
  EnterInsert,
  EnterProcessing,
  ExitProcessing,
  Update,
  Error(String),
  GPTResponse(CreateChatCompletionResponse)
}
