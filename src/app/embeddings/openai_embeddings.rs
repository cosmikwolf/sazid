use async_openai::{
  config::OpenAIConfig,
  error::OpenAIError,
  types::{CreateEmbeddingRequestArgs, CreateEmbeddingResponse},
};

use crate::components::session::create_openai_client;

#[derive(Clone)]
pub enum EmbeddingModel {
  // the most used OpenAI embedding model
  Ada002(OpenAIConfig),
}

impl ToString for EmbeddingModel {
  fn to_string(&self) -> String {
    match self {
      Self::Ada002(_) => "text-embedding-ada-002".to_string(),
    }
  }
}

async fn new_openai_embedding(
  openai_config: &OpenAIConfig,
  model: String,
  text: String,
) -> Result<CreateEmbeddingResponse, OpenAIError> {
  let client = create_openai_client(openai_config);
  let request = CreateEmbeddingRequestArgs::default().model(model).input(text).build()?;
  let response = client.embeddings().create(request).await?;
  for data in response.data.clone() {
    println!("[{}]: has embedding of length {}", data.index, data.embedding.len())
  }
  Ok(response)
}

pub async fn create_embedding(text: String, model: &EmbeddingModel) -> Result<CreateEmbeddingResponse, OpenAIError> {
  let model_name = model.to_string();
  match model {
    EmbeddingModel::Ada002(config) => new_openai_embedding(config, model_name, text).await,
  }
}
