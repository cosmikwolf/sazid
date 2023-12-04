#[cfg(test)]
mod vector_custom_type_tests {
  use sazid::app::embeddings::types::EmbeddingVector;
  use tokio_postgres::{Client, Error, NoTls};

  async fn setup_db() -> Result<Client, Box<dyn std::error::Error>> {
    let (client, connection) =
      tokio_postgres::connect("host=localhost user=postgres password=postgres-one-two-three-password", NoTls).await?;
    tokio::spawn(async move {
      if let Err(e) = connection.await {
        eprintln!("connection error: {}", e);
      }
    });
    Ok(client)
  }

  async fn teardown(client: &Client) -> Result<(), Error> {
    client.batch_execute("DROP TABLE IF EXISTS test_vectors;").await
  }

  #[tokio::test]
  async fn test_vector_custom_type_insert_and_retrieve_batch() -> Result<(), Box<dyn std::error::Error>> {
    let client = setup_db().await?;
    teardown(&client).await?;
    client.batch_execute("CREATE TABLE test_vectors (id SERIAL PRIMARY KEY, vec vector(3));").await?;

    let vector = EmbeddingVector::new(vec![1.0, 2.0, 3.0]);
    let batch = format!("INSERT INTO test_vectors (vec) VALUES ({})", vector.string_representation());
    println!("stmt: {:?}", batch);
    client.batch_execute(&batch).await?;
    let rows = client.simple_query("SELECT vec FROM test_vectors ORDER BY id DESC LIMIT 1").await?;
    let vectors = EmbeddingVector::from_simple_query_messages(&rows)?;

    teardown(&client).await?;
    assert_eq!(vectors, vec![EmbeddingVector { data: vec![1.0, 2.0, 3.0] }]);
    Ok(())
  }
}
