// Rust source file for generating embeddings using async_openai library
use std::error::Error;

use async_openai::{types::CreateEmbeddingRequestArgs, Client};

async fn _generate_embedding() -> Result<(), Box<dyn Error>> {
  let client = Client::new();

  // An embedding is a vector (list) of floating point numbers.
  // The distance between two vectors measures their relatedness.
  // Small distances suggest high relatedness and large distances suggest low relatedness.

  let request = CreateEmbeddingRequestArgs::default()
    .model("text-embedding-ada-002")
    .input([
      "Why do programmers hate nature? It has too many bugs.",
      "Why was the computer cold? It left its Windows open.",
    ])
    .build()?;

  let response = client.embeddings().create(request).await?;

  for data in response.data {
    println!("[{}]: has embedding of length {}", data.index, data.embedding.len())
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use insta::assert_yaml_snapshot;

  #[tokio::test]
  async fn test_embeddings_generation() {
    let client = Client::new();
    let request = CreateEmbeddingRequestArgs::default()
      .model("text-embedding-ada-002")
      .input([
        "Why do programmers hate nature? It has too many bugs.",
        "Why was the computer cold? It left its Windows open.",
      ])
      .build()
      .unwrap();

    let response = client.embeddings().create(request).await.unwrap();

    assert_yaml_snapshot!(response);
  }
}
