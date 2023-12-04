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
    let table_name = "test_vectors";
    let dimensions = 3;
    let create_table_query = format!(
      "CREATE TABLE IF NOT EXISTS {} (
        id bigserial PRIMARY KEY,
        text TEXT NOT NULL,
        embedding vector({}) NOT NULL
      );",
      table_name, dimensions
    );

    let drop_query = format!("DROP TABLE IF EXISTS {};", table_name);
    client.batch_execute(&drop_query).await?;
    client.batch_execute(&create_table_query).await?;
    let vector = EmbeddingVector::new(vec![1.0, 2.0, 3.0], "textomg".into());
    let batch = format!(
      "INSERT INTO {} (text, embedding) VALUES ('{}',{});",
      table_name,
      vector.data,
      vector.string_representation(),
    );
    println!("stmt: {:?}", batch);
    client.batch_execute(&batch).await?;
    let query = format!("SELECT * FROM {} ORDER BY id DESC LIMIT 1", table_name);
    let rows = client.simple_query(&query).await?;
    println!("rows: {:?}", rows);
    let vectors = EmbeddingVector::from_simple_query_messages(&rows)?;

    teardown(&client).await?;
    assert_eq!(vectors, vec![EmbeddingVector { data: "textomg".into(), embedding: vec![1.0, 2.0, 3.0] }]);
    Ok(())
  }
}
