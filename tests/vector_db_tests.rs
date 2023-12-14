#[cfg(test)]

mod vector_db_tests {
  use sazid::app::embeddings::vector_db::*;
  use sazid::app::{embeddings::types::FileEmbedding, errors::SazidError};
  use tokio_postgres::{Client, NoTls};

  // Helper function to set up the test database connection
  async fn setup_test_db() -> Result<VectorDB, SazidError> {
    let (client, connection) =
      tokio_postgres::connect("host=localhost user=postgres password=postgres-one-two-three-password", NoTls).await?;

    tokio::spawn(async move {
      if let Err(e) = connection.await {
        eprintln!("Connection error: {}", e);
      }
    });

    let vectordb = VectorDB { client, config: VectorDBConfig { optimize_threads: 4 } };

    vectordb.enable_extension().await?;
    Ok(vectordb)
  }

  // Shared cleanup function to be called after each test
  async fn cleanup_test_db(client: &Client, category_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Drop the sequence and table if they exist. The CASCADE option will take care of any dependent objects.
    let table_name = VectorDB::get_table_name(category_name);
    println!("dropping table_name: {:?}", table_name);
    client.execute(format!("DROP TABLE IF EXISTS {} CASCADE;", table_name).as_str(), &[]).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_list_embeddings_categories() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let category_names = ["a_test", "b_test"];
    // a method that will drop all tables that have a suffix of _embedding
    let query = "SELECT table_name FROM information_schema.tables WHERE table_name LIKE '%_embedding';";
    let rows = db.client.query(query, &[]).await?;
    println!("rows: {:#?}", rows);
    for row in rows {
      let table_name: String = row.get("table_name");
      let drop_string = format!("DROP TABLE IF EXISTS {} CASCADE;", table_name);
      println!("drop_string: {:?}", drop_string);
      db.client.batch_execute(&drop_string).await?;
    }
    // Insert texts with embeddings
    let embedding1 = FileEmbedding::new(vec![1.0_f64, 2.0_f64, 3.0_f64], "test".to_string(), category_names[0].into());
    let embedding2 = FileEmbedding::new(vec![2.0_f64, 2.0_f64, 3.0_f64], "test2".to_string(), category_names[1].into());
    db.insert_embedding(category_names[0], embedding1).await?;
    db.insert_embedding(category_names[1], embedding2).await?;
    let mut categories = db.list_embeddings_categories().await?;
    println!("categories: {:#?}", categories);
    cleanup_test_db(&db.client, category_names[0]).await?;
    cleanup_test_db(&db.client, category_names[1]).await?;

    // sort categories by alphabetical
    categories.sort();

    assert_eq!(categories.len(), 2, "Should retrieve 2 categories");
    assert_eq!(categories, ["a_test_embedding", "b_test_embedding"]);
    Ok(())
  }

  #[tokio::test]
  async fn test_search_similar_texts() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let category_name = "test";
    // Insert texts with embeddings
    cleanup_test_db(&db.client, category_name).await?;
    for i in 1..=5 {
      let text = format!("Example texxxxt {}", i);
      let embedding =
        FileEmbedding::new(vec![1.0_f64, 2.0_f64, 3.0_f64 + i as f64 / 1000000_f64], text, category_name.into());
      db.insert_embedding(category_name, embedding).await?;
    }

    // Query for a vector that is similar to the inserted vectors
    let query_embedding = vec![1.0, 2.0, 3.0];
    let similar_text_ids = db.search_similar_texts(category_name, &query_embedding, 5).await?;
    cleanup_test_db(&db.client, category_name).await?;

    // Since there are 5 vectors inserted, we expect to retrieve 5 similar texts
    assert_eq!(similar_text_ids.len(), 5, "Should retrieve 5 similar texts");

    Ok(())
  }

  #[tokio::test]
  async fn test_vector_insert_and_retrieve() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let category = "items_test";
    cleanup_test_db(&db.client, category).await?;
    let table_name = VectorDB::get_table_name(category);
    let vector = FileEmbedding::new(vec![1.0, 2.0, 3.0], "testtext".into(), category.into());

    db.insert_embedding(category, vector.clone()).await?;
    let query = format!("SELECT * FROM {} ORDER BY id DESC LIMIT 1", table_name);
    let rows = db.client.simple_query(&query).await?;
    println!("rows: {:?}", rows);
    let vectors = FileEmbedding::from_simple_query_messages(&rows, category)?;
    println!("vectors: {:#?}", vectors);
    assert_eq!(vector, vectors[0]);
    cleanup_test_db(&db.client, category).await?;
    Ok(())
  }
  // a method that generates a string that contains the current date and time with seconds
  fn get_current_time_string() -> String {
    let now = chrono::offset::Utc::now();
    now.format("%Y-%m-%d_%H-%M-%S").to_string()
  }

  #[tokio::test]
  async fn test_insert_text_and_retrieve_by_id() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let category_name = "b_test";
    cleanup_test_db(&db.client, category_name).await?;
    let text = get_current_time_string();
    let embedding = vec![0.1, 0.2, 0.3]; // Example embedding
    let vector = FileEmbedding::new(embedding, text.clone(), category_name.to_string());
    db.insert_embedding(category_name, vector).await?;

    // Retrieve the inserted text and embedding by ID
    let query = format!("SELECT id FROM {} ORDER BY id DESC LIMIT 1", VectorDB::get_table_name(category_name));
    println!("query: {:?}", query);
    let text_id: i64 = db.client.query_one(&query, &[]).await?.get("id");
    let retrieved_text = db.get_text_by_id(category_name, text_id as i64).await?;

    assert_eq!(text, retrieved_text, "Retrieved text does not match inserted text");

    cleanup_test_db(&db.client, category_name).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_enable_extension() -> Result<(), Box<dyn std::error::Error>> {
    let vectordb = setup_test_db().await?;
    vectordb.enable_extension().await?;
    // Verification step assumed
    Ok(())
  }

  #[tokio::test]
  async fn test_create_custom_index() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let category_name = "items_test";
    cleanup_test_db(&db.client, category_name).await?;
    let text = get_current_time_string();
    let embedding = vec![0.1, 0.2, 0.3]; // Example embedding
    let vector = FileEmbedding::new(embedding, text.clone(), category_name.to_string());
    db.insert_embedding(category_name, vector).await?;

    let index_type = "l2";
    // Corrected TOML configuration string
    let options = r#"indexing.flat.quantization.product.ratio = "x16""#;
    VectorDB::create_custom_index(&db.client, category_name, index_type, options).await?;

    cleanup_test_db(&db.client, category_name).await?;
    // Verification step assumed
    Ok(())
  }

  #[tokio::test]
  async fn test_set_search_option() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let category_name = "items_test";
    VectorDB::set_search_option(&db.client, "vectors.k", "10").await?;
    // Verification step assumed
    cleanup_test_db(&db.client, category_name).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_query_knn() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let category_name = "items_test";
    let table_name = VectorDB::get_table_name(category_name);
    cleanup_test_db(&db.client, category_name).await?;
    db.create_category_table(category_name, 3, false).await?;

    for i in 0..5 {
      let text = get_current_time_string();
      let vector = vec![0.1 + i as f64, 0.2, 0.3]; // Example embedding
      let embedding = FileEmbedding::new(vector, text.clone(), category_name.to_string());
      db.insert_embedding(category_name, embedding).await?;
    }

    // Convert &[f64] to a string representation PostgreSQL can understand
    let vector_as_string = [1.0, 2.0, 3.0].iter().map(|val| val.to_string()).collect::<Vec<String>>().join(",");
    let query = format!(
      "SELECT id, embedding::text FROM {} ORDER BY embedding <-> ARRAY[{}]::real[] LIMIT $1;",
      table_name, vector_as_string
    );

    // Execute the query with the proper limit parameter
    let rows = db.client.query(&query, &[&(5i64)]).await?;

    let mut results = Vec::new();
    for row in rows {
      let _id: i64 = row.get(0);
      let embedding_text: String = row.get(1);
      // Parse the text back into a vector or an appropriate format for further use
      results.push(embedding_text);
    }
    assert_eq!(results.len(), 5);

    cleanup_test_db(&db.client, category_name).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_create_vector_table() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db().await?;
    let category = "items_test";
    cleanup_test_db(&db.client, category).await?;

    db.create_category_table(category, 3, false).await?;
    // Verification step assumed
    cleanup_test_db(&db.client, category).await?;
    Ok(())
  }

  #[tokio::test]
  async fn test_get_indexing_progress() -> Result<(), Box<dyn std::error::Error>> {
    let vectordb = setup_test_db().await?;
    vectordb.enable_extension().await?;
    let _progress = VectorDB::get_indexing_progress(&vectordb.client).await?;
    // Assertions based on expected indexing progress
    vectordb.client.batch_execute("DROP TABLE IF EXISTS items;").await?;
    Ok(())
  }

  // Additional tests for edge cases, error conditions, and other methods in vector_db.rs...
}
