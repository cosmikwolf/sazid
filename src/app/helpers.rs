use async_openai::types::{
  ChatCompletionResponseStreamMessage, ChatCompletionStreamResponseDelta, FinishReason, FunctionCallStream,
};

use super::errors::ParseError;

pub fn concatenate_option_strings(a: Option<String>, b: Option<String>) -> Option<String> {
  match (a, b) {
    (Some(a_str), Some(b_str)) => Some(a_str + &b_str), // Concatenate if both are Some
    (Some(a_str), None) => Some(a_str),                 // Only a is Some
    (None, Some(b_str)) => Some(b_str),                 // Only b is Some
    (None, None) => None,                               // Both are None
  }
}

pub fn concatenate_function_call_streams(
  call1: Option<FunctionCallStream>,
  call2: Option<FunctionCallStream>,
) -> Option<FunctionCallStream> {
  match (call1, call2) {
    (Some(fc1), Some(fc2)) => {
      Some(FunctionCallStream {
        // Choose the first `Some` or `None` if both are `None`
        name: concatenate_option_strings(fc1.name, fc2.name),
        arguments: concatenate_option_strings(fc1.arguments, fc2.arguments),
      })
    },
    (Some(fc), None) | (None, Some(fc)) => Some(fc),
    (None, None) => None,
  }
}

pub fn concatenate_stream_delta(
  delta1: ChatCompletionStreamResponseDelta,
  delta2: ChatCompletionStreamResponseDelta,
) -> ChatCompletionStreamResponseDelta {
  ChatCompletionStreamResponseDelta {
    role: delta1.role,
    content: concatenate_option_strings(delta1.content, delta2.content),
    function_call: concatenate_function_call_streams(delta1.function_call, delta2.function_call),
  }
}

pub fn concatenate_finish_reason(
  finish_reason1: Option<FinishReason>,
  finish_reason2: Option<FinishReason>,
) -> Result<Option<FinishReason>, ParseError> {
  match (finish_reason1, finish_reason2) {
    (Some(_), Some(_)) => Err(ParseError::new("Cannot concatenate two finish reasons")),
    (Some(fr), None) => Ok(Some(fr)),
    (None, Some(fr)) => Ok(Some(fr)),
    (None, None) => Ok(None), // todo: handle this case
  }
}

pub fn concatenate_stream_response_messages(
  sr1: &ChatCompletionResponseStreamMessage,
  sr2: &ChatCompletionResponseStreamMessage,
) -> ChatCompletionResponseStreamMessage {
  ChatCompletionResponseStreamMessage {
    index: sr1.index,
    delta: concatenate_stream_delta(sr1.delta.clone(), sr2.delta.clone()),
    finish_reason: concatenate_finish_reason(sr1.finish_reason, sr2.finish_reason).unwrap(),
  }
}

pub fn collate_stream_response_vec(
  new_srvec: Vec<ChatCompletionResponseStreamMessage>,
  existing_srvec: &mut Vec<ChatCompletionResponseStreamMessage>,
) {
  // trace_dbg!("add_message: supplimental delta \n{:?}\n{:?}", new_srvec, existing_srvec);
  new_srvec.iter().for_each(|new_sr| {
    if !existing_srvec.iter_mut().any(|existing_sr| {
      if existing_sr.index == new_sr.index {
        *existing_sr = concatenate_stream_response_messages(existing_sr, new_sr);
        true
      } else {
        false
      }
    }) {
      existing_srvec.push(new_sr.clone());
    }
  });
}
use std::fs::{self, DirEntry};
use std::io;
use std::path::Path;
use std::time::SystemTime;

pub fn list_files_ordered_by_date<P: AsRef<Path>>(path: P) -> io::Result<Vec<DirEntry>> {
  let mut entries: Vec<DirEntry> = fs::read_dir(path)?.filter_map(|entry| entry.ok()).collect();

  entries.sort_by_key(|entry| entry.metadata().and_then(|meta| meta.modified()).unwrap_or(SystemTime::UNIX_EPOCH));

  Ok(entries)
}
