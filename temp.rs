// please write me a class function for a Session object
// that will iterate through self.transactions
// it should skip transactions that share the same id as one that has a completed = true, in a way that so this function can be run, and will not re-render transactions that have already been completed and stored.
// The result should contain:
// - a RenderedChatTransaction for each Request id
// - a RenderedChatTransaction for each Response id
// - a RenderedChatTransaction for each StreamResponse id
// StreamResponse transactions should be combined into a RenderedChatTransaction by combining StreamResponseDeltas by id and choice.
// combined stream response deltas should be combined by setting the Role if it exist in teh stream response delta, and by concatenating content, as well as function_cal name and arguments
// The resulting RenderedChatTransaction should be added to self.rendered_transactions
// use the following code to help you understand the data structures
// do not abbreviate and do not use code stubs such as // handle this here and // do this here

pub struct Transaction {
  pub id: String,
  pub original: Vec<ChatTransaction>,
  pub rendered: Option<RenderedChatTransaction>,
  pub completed: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct ChatCompletionResponseMessage {
  pub role: Role,
  pub content: Option<String>,
  pub function_call: Option<FunctionCall>,
}
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct ChatChoice {
  pub index: u32,
  pub message: ChatCompletionResponseMessage,
  pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize)]
pub struct CreateChatCompletionResponse {
  pub id: String,
  pub object: String,
  pub created: u32,
  pub model: String,
  pub usage: Option<Usage>,
  pub choices: Vec<ChatChoice>,
}

pub struct ChatCompletionStreamResponseDelta {
  pub role: Option<Role>,
  pub content: Option<String>,
  pub function_call: Option<FunctionCallStream>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct ChatCompletionResponseStreamMessage {
  pub index: u32,
  pub delta: ChatCompletionStreamResponseDelta,
  pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize)]
pub struct CreateChatCompletionStreamResponse {
  pub id: String,
  pub object: String,
  pub created: u32,
  pub model: String,
  pub choices: Vec<ChatCompletionResponseStreamMessage>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ChatTransaction {
  Request(CreateChatCompletionRequest),
  Response(CreateChatCompletionResponse),
  StreamResponse(CreateChatCompletionStreamResponse),
}
#[derive(Clone, Default)]
pub struct RenderedChatTransaction {
  pub id: Option<String>,
  pub choices: Vec<RenderedChatMessage>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct Session {
  pub transactions: Vec<Transaction>,
}

#[derive(Clone, Default)]
pub struct RenderedChatMessage {
  pub role: Option<Role>,
  pub content: Option<String>,
  pub function_call: Option<RenderedFunctionCall>,
  pub finish_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RenderedFunctionCall {
  pub name: Option<String>,
  pub arguments: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct FunctionCall {
  pub name: String,
  pub arguments: String,
}

pub struct ChatCompletionRequestMessage {
  pub role: Role,
  pub content: Option<String>,
  pub name: Option<String>,
  pub function_call: Option<FunctionCall>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct ChatCompletionResponseMessage {
  pub role: Role,
  pub content: Option<String>,
  pub function_call: Option<FunctionCall>,
}

pub struct CreateChatCompletionRequest {
  pub model: String,
  pub messages: Vec<ChatCompletionRequestMessage>, // min: 1
  pub functions: Option<Vec<ChatCompletionFunctions>>,
  pub function_call: Option<ChatCompletionFunctionCall>,
  pub temperature: Option<f32>, // min: 0, max: 2, default: 1,
  pub top_p: Option<f32>,       // min: 0, max: 1, default: 1
  pub n: Option<u8>,            // min:1, max: 128, default: 1
  pub stream: Option<bool>,
  pub stop: Option<Stop>,
  pub max_tokens: Option<u16>,
  pub presence_penalty: Option<f32>,  // min: -2.0, max: 2.0, default 0
  pub frequency_penalty: Option<f32>, // min: -2.0, max: 2.0, default: 0
  pub logit_bias: Option<HashMap<String, serde_json::Value>>, // default: null
  pub user: Option<String>,
}
