use async_openai::types::{
  ChatCompletionMessageToolCall, ChatCompletionMessageToolCallChunk, ChatCompletionRequestAssistantMessage,
  ChatCompletionResponseStreamMessage, ChatCompletionStreamResponseDelta, ChatCompletionToolType,
  CreateChatCompletionResponse, CreateChatCompletionStreamResponse, FinishReason, FunctionCall, FunctionCallStream,
  Role,
};

use crate::trace_dbg;

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

pub fn concatenate_option_vecs<T>(a: Option<Vec<T>>, b: Option<Vec<T>>) -> Option<Vec<T>> {
  match (a, b) {
    (Some(a_vec), Some(b_vec)) => Some(a_vec.into_iter().chain(b_vec).collect()),
    (Some(a_vec), None) => Some(a_vec),
    (None, Some(b_vec)) => Some(b_vec),
    (None, None) => None,
  }
}

pub fn concatenate_tool_call_chunks(
  chunk1: &ChatCompletionMessageToolCallChunk,
  chunk2: &ChatCompletionMessageToolCallChunk,
) -> ChatCompletionMessageToolCallChunk {
  ChatCompletionMessageToolCallChunk {
    index: chunk1.index,
    id: concatenate_option_strings(chunk1.id.clone(), chunk2.id.clone()),
    r#type: chunk2.r#type.clone(),
    function: concatenate_function_call_streams(chunk1.function.clone(), chunk2.function.clone()),
  }
}

pub fn concatenate_stream_delta(
  delta1: ChatCompletionStreamResponseDelta,
  delta2: ChatCompletionStreamResponseDelta,
) -> ChatCompletionStreamResponseDelta {
  ChatCompletionStreamResponseDelta {
    role: delta1.role,
    content: concatenate_option_strings(delta1.content, delta2.content),
    tool_calls: concatenate_option_vecs::<ChatCompletionMessageToolCallChunk>(delta1.tool_calls, delta2.tool_calls),
    function_call: concatenate_function_call_streams(delta1.function_call, delta2.function_call),
  }
}

pub fn concatenate_create_chat_completion_stream_response(
  sr1: &CreateChatCompletionStreamResponse,
  sr2: &CreateChatCompletionStreamResponse,
) -> Result<CreateChatCompletionStreamResponse, ParseError> {
  if sr1.id != sr2.id {
    Err(ParseError::new("Cannot concatenate two stream responses with different ids"))
  } else {
    let mut combined_choices = Vec::new();
    combined_choices.extend(sr1.choices.clone());
    combined_choices.extend(sr2.choices.clone());
    Ok(CreateChatCompletionStreamResponse {
      id: sr1.id.clone(),
      choices: combined_choices,
      created: sr2.created,
      model: sr2.model.clone(),
      system_fingerprint: sr2.system_fingerprint.clone(),
      object: sr2.object.clone(),
    })
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
) -> Result<ChatCompletionResponseStreamMessage, ParseError> {
  if sr1.index != sr2.index {
    Err(ParseError::new("Cannot concatenate two stream responses with different indexes"))
  } else {
    Ok(ChatCompletionResponseStreamMessage {
      index: sr1.index,
      delta: concatenate_stream_delta(sr1.delta.clone(), sr2.delta.clone()),
      finish_reason: concatenate_finish_reason(sr1.finish_reason, sr2.finish_reason).unwrap(),
    })
  }
}

pub fn convert_tool_chunk_to_tool_call(chunk: &ChatCompletionMessageToolCallChunk) -> ChatCompletionMessageToolCall {
  ChatCompletionMessageToolCall {
    id: chunk.id.clone().unwrap_or("".to_string()),
    r#type: chunk.r#type.clone().unwrap_or_default(),
    function: FunctionCall {
      name: chunk.function.clone().unwrap().name.unwrap_or("".to_string()),
      arguments: chunk.function.clone().unwrap().arguments.unwrap_or("".to_string()),
    },
  }
}
pub fn append_tool_call_chunk_to_tool_call(
  call: &mut ChatCompletionMessageToolCall,
  chunk: &ChatCompletionMessageToolCallChunk,
) {
  let (name, arguments) = match &chunk.function {
    Some(fc) => (fc.name.clone().unwrap_or("".to_string()), fc.arguments.clone().unwrap_or("".to_string())),
    None => ("".to_string(), "".to_string()),
  };
  call.id += &chunk.id.clone().unwrap_or("".to_string());
  call.function.name += name.as_str();
  call.function.arguments += arguments.as_str();
}

pub fn collate_tool_call_chunks_into_tool_calls(
  tc_chunks: Vec<ChatCompletionMessageToolCallChunk>,
) -> Option<Vec<ChatCompletionMessageToolCall>> {
  let mut tc_calls: Vec<ChatCompletionMessageToolCall> = Vec::new();
  tc_chunks.iter().for_each(|tc_chunk| {
    // trace_dbg!("tc_chunk: {:?}", tc_chunk);
    // trace_dbg!("tc_calls.len(): {:?}", tc_calls.len());
    match tc_calls.get_mut(tc_chunk.index as usize) {
      Some(tc_call) => append_tool_call_chunk_to_tool_call(tc_call, tc_chunk),
      None => tc_calls.push(convert_tool_chunk_to_tool_call(tc_chunk)),
    }
  });
  if tc_calls.is_empty() {
    None
  } else {
    Some(tc_calls)
  }
}

pub fn get_assistant_message_from_create_chat_completion_stream_response(
  choice_index: usize,
  srvec: &[CreateChatCompletionStreamResponse],
) -> Result<ChatCompletionRequestAssistantMessage, ParseError> {
  let mut smvec = Vec::new();
  srvec.iter().for_each(|sr| {
    sr.choices
      .iter()
      .filter(|choice| choice.index as usize == choice_index)
      .for_each(|choice| smvec.push(choice.clone()))
  });
  fold_stream_responses_into_assistant_message(smvec)
}

pub fn get_assistant_message_from_create_chat_completion_response(
  choice_index: usize,
  response: &CreateChatCompletionResponse,
) -> Result<ChatCompletionRequestAssistantMessage, ParseError> {
  if choice_index >= response.choices.len() {
    Err(ParseError::new(format!("Choice index {} out of range", choice_index).as_str()))
  } else {
    Ok(ChatCompletionRequestAssistantMessage {
      role: Role::Assistant,
      content: response.choices[choice_index].message.content.clone(),
      function_call: None,
      tool_calls: response.choices[choice_index].message.tool_calls.clone(),
    })
  }
}
pub fn fold_stream_responses_into_assistant_message(
  smvec: Vec<ChatCompletionResponseStreamMessage>,
) -> Result<ChatCompletionRequestAssistantMessage, ParseError> {
  let concatenated_message =
    smvec.iter().skip(1).try_fold(smvec[0].clone(), |acc, sr| concatenate_stream_response_messages(&acc, sr))?;

  // let function_call = match concatenated_message.delta.function_call {
  //   Some(fc) => {
  //     Some(FunctionCall { name: fc.name.unwrap_or("".to_string()), arguments: fc.arguments.unwrap_or("".to_string()) })
  //   },
  //   None => None,
  // };

  Ok(ChatCompletionRequestAssistantMessage {
    role: Role::Assistant,
    content: concatenated_message.delta.content,
    function_call: None,
    tool_calls: collate_tool_call_chunks_into_tool_calls(concatenated_message.delta.tool_calls.unwrap_or(Vec::new())),
  })
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
