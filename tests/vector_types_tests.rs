#[cfg(test)]
mod vector_custom_type_tests {
  use sazid::app::embeddings::types::EmbeddingVector;
  use tokio_postgres::{types::ToSql, Client, Error, NoTls};

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
  async fn test_vector_custom_type_as_text_insert_and_retrieve() -> Result<(), Box<dyn std::error::Error>> {
    let client = setup_db().await?;
    teardown(&client).await?;
    client.batch_execute("CREATE TABLE test_vectors (id SERIAL PRIMARY KEY, vec TEXT);").await?;

    let vector = EmbeddingVector::new(vec![1.0, 2.0, 3.0]);

    let stmt = client.prepare("INSERT INTO test_vectors (vec) VALUES ($1)").await?;
    let vector_string = format!("[{}]", vector.data.iter().map(|num| num.to_string()).collect::<Vec<_>>().join(","));
    client.execute(&stmt, &[&vector_string]).await?;
    let stmt = client.prepare("SELECT vec FROM test_vectors ORDER BY id DESC LIMIT 1").await?;
    let rows = client.query(&stmt, &[]).await?;
    println!("rows: {:?}", rows);
    let retrieved_data: &str = rows[0].get(0);
    let retrieved_vector: Vec<f64> =
      retrieved_data[1..retrieved_data.len() - 1].split(',').map(|num_str| num_str.parse().unwrap()).collect();
    assert_eq!(vector.data, retrieved_vector);

    teardown(&client).await?;
    Ok(())
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

    let stmt = client.prepare("SELECT vec FROM test_vectors ORDER BY id DESC LIMIT 1").await?;
    let rows = client.query(&stmt, &[]).await?;

    let retrieved_data: &str = rows[0].get(0);
    let retrieved_vector: Vec<f64> =
      retrieved_data[1..retrieved_data.len() - 1].split(',').map(|num_str| num_str.parse().unwrap()).collect();
    assert_eq!(vector.data, retrieved_vector);

    teardown(&client).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_vector_custom_type_insert_and_retrieve() -> Result<(), Box<dyn std::error::Error>> {
    let client = setup_db().await?;
    teardown(&client).await?;
    client.batch_execute("CREATE TABLE test_vectors (id SERIAL PRIMARY KEY, vec vector(3));").await?;

    let vector = EmbeddingVector::new(vec![1.0, 2.0, 3.0]);
    let stmt = client.prepare("INSERT INTO test_vectors (vec) VALUES ($1)").await?;
    client.execute(&stmt, &[&vector as &(dyn ToSql + Sync)]).await?;
    let stmt = client.prepare("SELECT vec FROM test_vectors ORDER BY id DESC LIMIT 1").await?;
    let rows = client.query(&stmt, &[]).await?;

    let retrieved_data: &str = rows[0].get(0);
    let retrieved_vector: Vec<f64> =
      retrieved_data[1..retrieved_data.len() - 1].split(',').map(|num_str| num_str.parse().unwrap()).collect();
    assert_eq!(vector.data, retrieved_vector);

    teardown(&client).await?;
    Ok(())
  }
}
