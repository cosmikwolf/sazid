#[cfg(test)]
mod vector_custom_type_tests {
  use sazid::app::database::vector_custom_type::Vector;
  use tokio_postgres::types::{FromSql, ToSql};
  use tokio_postgres::{Client, NoTls};

  async fn setup_db() -> Result<Client, Box<dyn std::error::Error>> {
    let (client, connection) = tokio_postgres::connect("host=localhost user=postgres dbname=postgres", NoTls).await?;
    tokio::spawn(async move {
      if let Err(e) = connection.await {
        eprintln!("Connection error: {}", e);
      }
    });
    Ok(client)
  }

  #[tokio::test]
  async fn test_vector_custom_type_insert_and_retrieve() -> Result<(), Box<dyn std::error::Error>> {
    let client = setup_db().await?;
    let vector = Vector::new(vec![1.0, 2.0, 3.0]);

    // Insert vector into a test table
    client.batch_execute("DROP TABLE IF EXISTS test_vectors;").await?;
    client.batch_execute("CREATE TABLE test_vectors (id SERIAL PRIMARY KEY, vec _float8);").await?;
    client.execute("INSERT INTO test_vectors (vec) VALUES ($1)", &[&vector as &(dyn ToSql + Sync)]).await?;

    // Retrieve the vector
    let row = client.query_one("SELECT vec FROM test_vectors LIMIT 1", &[]).await?;
    let retrieved_vector: Vector = row.get(0);

    // Assert that the retrieved vector matches the original
    assert_eq!(vector, retrieved_vector);

    Ok(())
  }
}
