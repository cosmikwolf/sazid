
// use std::error::Error;

// use async_openai::{types::CreateEmbeddingRequestArgs, Client};

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn Error>> {
//     let client = Client::new();

//     // An embedding is a vector (list) of floating point numbers.
//     // The distance between two vectors measures their relatedness.
//     // Small distances suggest high relatedness and large distances suggest low relatedness.

//     let request = CreateEmbeddingRequestArgs::default()
//         .model("text-embedding-ada-002")
//         .input([
//             "Why do programmers hate nature? It has too many bugs.",
//             "Why was the computer cold? It left its Windows open.",
//         ])
//         .build()?;

//     let response = client.embeddings().create(request).await?;

//     for data in response.data {
//         println!(
//             "[{}]: has embedding of length {}",
//             data.index,
//             data.embedding.len()
//         )
//     }

//     Ok(())
// }
// use the above code to create a create embedding request function that will take the following arguments:
// model: str
// input: List[str]

// and return the following:
// response: CreateEmbeddingResponse

