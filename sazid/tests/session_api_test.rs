#[cfg(test)]
mod tests {
  use async_openai::types::{
    ChatChoice, ChatCompletionResponseMessage, ChatCompletionResponseStreamMessage, ChatCompletionStreamResponseDelta,
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse, FinishReason, FunctionCallStream, Role,
  };
  use ntest::timeout;
  use sazid::action;
  use sazid::app::messages::{ChatMessage, ChatResponse, ChatResponseSingleMessage};
  use sazid::components::session::*;
  use tokio::sync::mpsc;
  // write a test for Session::add_message
  #[tokio::test]
  #[timeout(10000)]
  pub async fn test_add_message() {
    let mut session = Session::new();
    let (tx, _rx) = mpsc::unbounded_channel::<action::Action>();
    session.data.add_message(ChatMessage::PromptMessage(session.config.prompt_message()));
    assert_eq!(session.data.messages.len(), 1);
    assert_eq!(session.data.messages[0].message, ChatMessage::PromptMessage(session.config.prompt_message()));
    // Create a mock response from OpenAI
    let response = CreateChatCompletionResponse {
      id: "cmpl-3fZzT7q5Y3zJ5Jp9Dq3qX8s0".to_string(),
      object: "text_completion".to_string(),
      usage: None,
      created: 1613908771,
      model: "davinci:2020-05-03".to_string(),
      choices: vec![ChatChoice {
        index: 0,
        message: ChatCompletionResponseMessage {
          role: Role::User,
          content: Some("test response data".to_string()),
          function_call: None,
        },
        finish_reason: Some(FinishReason::Stop),
      }],
    };
    let stream_responses = vec![
      CreateChatCompletionStreamResponse {
        id: "cmpl-3fZzT7q5Y3zJ5Jp9Dq3qX8s0".to_string(),
        object: "text_completion".to_string(),
        created: 1613908771,
        model: "davinci:2020-05-03".to_string(),
        choices: vec![ChatCompletionResponseStreamMessage {
          index: 0,
          delta: ChatCompletionStreamResponseDelta { role: Some(Role::Assistant), content: None, function_call: None },
          finish_reason: None,
        }],
      },
      CreateChatCompletionStreamResponse {
        id: "cmpl-3fZzT7q5Y3zJ5Jp9Dq3qX8s0".to_string(),
        object: "text_completion".to_string(),
        created: 1613908772,
        model: "davinci:2020-05-03".to_string(),
        choices: vec![ChatCompletionResponseStreamMessage {
          index: 0,
          delta: ChatCompletionStreamResponseDelta {
            role: None,
            content: Some("two".to_string()),
            function_call: None,
          },
          finish_reason: None,
        }],
      },
      CreateChatCompletionStreamResponse {
        id: "cmpl-3fZzT7q5Y3zJ5Jp9Dq3qX8s0".to_string(),
        object: "text_completion".to_string(),
        created: 1613908772,
        model: "davinci:2020-05-03".to_string(),
        choices: vec![ChatCompletionResponseStreamMessage {
          index: 0,
          delta: ChatCompletionStreamResponseDelta {
            role: None,
            content: Some("three".to_string()),
            function_call: None,
          },
          finish_reason: None,
        }],
      },
      CreateChatCompletionStreamResponse {
        id: "cmpl-3fZzT7q5Y3zJ5Jp9Dq3qX8s0".to_string(),
        object: "text_completion".to_string(),
        created: 1613908773,
        model: "davinci:2020-05-03".to_string(),
        choices: vec![ChatCompletionResponseStreamMessage {
          index: 0,
          delta: ChatCompletionStreamResponseDelta { role: None, content: None, function_call: None },
          finish_reason: Some(FinishReason::Stop),
        }],
      },
    ];

    let stream_responses_with_function_calls = vec![
      CreateChatCompletionStreamResponse {
        id: "cmpl-3fZzT7q5Y3zJ5Jp9Dq3qX8s0".to_string(),
        object: "text_completion".to_string(),
        created: 1613908771,
        model: "gpt-3.5-turbo".to_string(),
        choices: vec![ChatCompletionResponseStreamMessage {
          index: 0,
          delta: ChatCompletionStreamResponseDelta {
            role: Some(Role::Assistant),
            content: None,
            function_call: Some(FunctionCallStream { name: Some("file_search".to_string()), arguments: None }),
          },
          finish_reason: None,
        }],
      },
      CreateChatCompletionStreamResponse {
        id: "cmpl-3fZzT7q5Y3zJ5Jp9Dq3qX8s0".to_string(),
        object: "text_completion".to_string(),
        created: 1613908772,
        model: "gpt-3.5-turbo".to_string(),
        choices: vec![ChatCompletionResponseStreamMessage {
          index: 0,
          delta: ChatCompletionStreamResponseDelta {
            role: None,
            content: None,
            function_call: Some(FunctionCallStream { name: None, arguments: Some("{ ".to_string()) }),
          },
          finish_reason: None,
        }],
      },
      CreateChatCompletionStreamResponse {
        id: "cmpl-3fZzT7q5Y3zJ5Jp9Dq3qX8s0".to_string(),
        object: "text_completion".to_string(),
        created: 1613908772,
        model: "gpt-3.5-turbo".to_string(),
        choices: vec![ChatCompletionResponseStreamMessage {
          index: 0,
          delta: ChatCompletionStreamResponseDelta {
            role: None,
            content: None,
            function_call: Some(FunctionCallStream { name: None, arguments: Some("src }".to_string()) }),
          },
          finish_reason: None,
        }],
      },
      CreateChatCompletionStreamResponse {
        id: "cmpl-3fZzT7q5Y3zJ5Jp9Dq3qX8s0".to_string(),
        object: "text_completion".to_string(),
        created: 1613908773,
        model: "davinci:2020-05-03".to_string(),
        choices: vec![ChatCompletionResponseStreamMessage {
          index: 0,
          delta: ChatCompletionStreamResponseDelta { role: None, content: None, function_call: None },
          finish_reason: Some(FinishReason::FunctionCall),
        }],
      },
    ];
    Session::response_handler(tx.clone(), ChatResponse::Response(response)).await;
    assert_eq!(session.data.messages.len(), 2);
    Session::response_handler(tx.clone(), ChatResponse::StreamResponse(stream_responses[0].clone())).await;
    assert_eq!(session.data.messages.len(), 3);
    if let ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(msg)) =
      &session.data.messages[2].message
    {
      assert_eq!(msg[0].delta.role, Some(Role::Assistant));
      assert_eq!(msg.len(), 1);
      assert_eq!(msg[0].delta.content, None);
      assert_eq!(msg[0].finish_reason, None);
    } else {
      panic!("Expected ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(msg))");
    }
    Session::response_handler(tx.clone(), ChatResponse::StreamResponse(stream_responses[1].clone())).await;
    assert_eq!(session.data.messages.len(), 3);
    if let ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(msg)) =
      &session.data.messages[2].message
    {
      assert_eq!(msg[0].delta.role, Some(Role::Assistant));
      assert_eq!(msg[0].delta.content, Some("two".to_string()));
      assert_eq!(msg[0].finish_reason, None);
    } else {
      panic!("Expected ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(msg))");
    }
    Session::response_handler(tx.clone(), ChatResponse::StreamResponse(stream_responses[2].clone())).await;
    assert_eq!(session.data.messages.len(), 3);
    if let ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(msg)) =
      &session.data.messages[2].message
    {
      assert_eq!(msg[0].delta.role, Some(Role::Assistant));
      assert_eq!(msg[0].delta.content, Some("twothree".to_string()));
      assert_eq!(msg[0].finish_reason, None);
    } else {
      panic!("Expected ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(msg))");
    }
    Session::response_handler(tx.clone(), ChatResponse::StreamResponse(stream_responses[3].clone())).await;
    assert_eq!(session.data.messages.len(), 3);
    if let ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(msg)) =
      &session.data.messages[2].message
    {
      assert_eq!(msg[0].delta.role, Some(Role::Assistant));
      assert_eq!(msg[0].delta.content, Some("twothree".to_string()));
      assert_eq!(msg[0].finish_reason, Some(FinishReason::Stop));
    } else {
      panic!("Expected ChatMessage::ChatCompletionRequestMessage(ChatResponseSingleMessage::StreamResponse(msg))");
    }
    assert!(session.data.messages[1].finished);
    Session::response_handler(
      tx.clone(),
      ChatResponse::StreamResponse(stream_responses_with_function_calls[0].clone()),
    )
    .await;
    assert_eq!(session.data.messages.len(), 4);
    if let ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(msg)) =
      &session.data.messages[3].message
    {
      assert_eq!(msg[0].delta.role, Some(Role::Assistant));
      assert_eq!(msg[0].delta.content, None);
      assert_eq!(msg[0].finish_reason, None);
      assert_eq!(
        msg[0].delta.function_call,
        Some(FunctionCallStream { name: Some("file_search".to_string()), arguments: None })
      );
    } else {
      panic!("Expected ChatMessage::ChatCompletionRequestMessage(ChatResponseSingleMessage::StreamResponse(msg))");
    }
    Session::response_handler(
      tx.clone(),
      ChatResponse::StreamResponse(stream_responses_with_function_calls[1].clone()),
    )
    .await;
    assert_eq!(session.data.messages.len(), 4);
    if let ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(msg)) =
      &session.data.messages[3].message
    {
      assert_eq!(msg[0].delta.role, Some(Role::Assistant));
      assert_eq!(msg[0].delta.content, None);
      assert_eq!(msg[0].finish_reason, None);
      assert_eq!(
        msg[0].delta.function_call,
        Some(FunctionCallStream { name: Some("file_search".to_string()), arguments: Some("{ ".to_string()) })
      );
    } else {
      panic!("Expected ChatMessage::ChatCompletionRequestMessage(ChatResponseSingleMessage::StreamResponse(msg))");
    }
    Session::response_handler(
      tx.clone(),
      ChatResponse::StreamResponse(stream_responses_with_function_calls[2].clone()),
    )
    .await;
    assert_eq!(session.data.messages.len(), 4);
    if let ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(msg)) =
      &session.data.messages[3].message
    {
      assert_eq!(msg[0].delta.role, Some(Role::Assistant));
      assert_eq!(msg[0].delta.content, None);
      assert_eq!(msg[0].finish_reason, None);
      assert_eq!(
        msg[0].delta.function_call,
        Some(FunctionCallStream { name: Some("file_search".to_string()), arguments: Some("{ src }".to_string()) })
      );
    } else {
      panic!("Expected ChatMessage::ChatCompletionRequestMessage(ChatResponseSingleMessage::StreamResponse(msg))");
    }
    Session::response_handler(
      tx.clone(),
      ChatResponse::StreamResponse(stream_responses_with_function_calls[3].clone()),
    )
    .await;
    assert_eq!(session.data.messages.len(), 4);
    if let ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(msg)) =
      &session.data.messages[3].message
    {
      assert_eq!(msg[0].delta.role, Some(Role::Assistant));
      assert_eq!(msg[0].delta.content, None);
      assert_eq!(msg[0].finish_reason, Some(FinishReason::FunctionCall));
      assert_eq!(
        msg[0].delta.function_call,
        Some(FunctionCallStream { name: Some("file_search".to_string()), arguments: Some("{ src }".to_string()) })
      );
    } else {
      panic!("Expected ChatMessage::ChatCompletionRequestMessage(ChatResponseSingleMessage::StreamResponse(msg))");
    }
    insta::assert_yaml_snapshot!(&session.data);
    insta::assert_yaml_snapshot!(&session.view.rendered_text.to_string());
  }
}
