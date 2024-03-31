#[cfg(test)]
mod tests {
  use super::*;
  use diesel::{Connection, RunQueryDsl};
  use diesel_async::{AsyncConnection, AsyncPgConnection};
  use sazid::app::database::types::*;

  async fn establish_connection() -> AsyncPgConnection {
    AsyncPgConnection::establish(&dotenv::var("TEST_DATABASE_URL").unwrap())
      .await
      .unwrap()
  }

  fn setup_test_container(connection: &AsyncPgConnection) -> PagesContainer {
    // Hypothetical function to create and return test PagesContainer
    // assuming pages_containers::dsl and InsertablePagesContainer are in scope
    diesel::insert_into(pages_containers::table)
      .values(InsertablePagesContainer {
        filepath: "test/filepath".into(),
        checksum: "checksum".into(),
      })
      .get_result(connection)
      .unwrap()
  }

  fn setup_test_textfile_embedding(
    connection: &AsyncPgConnection,
    container_id: i64,
  ) -> TextFileEmbedding {
    // Hypothetical function to create and return test TextFileEmbedding
    // assuming embeddings::dsl and InsertableTextFileEmbedding are in scope
    diesel::insert_into(embeddings::table)
      .values(InsertableTextFileEmbedding {
        pages_container_id: container_id,
        content: "test content".into(),
        checksum: "checksum".into(),
        page_number: 1,
        embedding: vec![], // Replace with a valid Vector
      })
      .get_result(connection)
      .unwrap()
  }

  #[tokio::test]
  async fn test_page_count() {
    let connection = establish_connection().await;
    reset_database(&connection).await;

    let test_container = setup_test_container(&connection);
    let page_count = test_container.page_count(&mut connection).await.unwrap();
    assert_eq!(page_count, expected_count); // Replace `expected_count` with correct value
  }

  #[tokio::test]
  async fn test_get_next_page() {
    let connection = establish_connection().await;
    reset_database(&connection).await;

    let test_container = setup_test_container(&connection);
    let test_textfile_embedding =
      setup_test_textfile_embedding(&connection, test_container.id);
    let next_page_option =
      test_textfile_embedding.get_next_page(&mut connection).await.unwrap();

    assert!(next_page_option.is_some(), "Next page does not exist.");
    // More assertions can be added based on expected data
  }

  #[tokio::test]
  async fn test_get_previous_page() {
    let connection = establish_connection().await;
    reset_database(&connection).await;

    let test_container = setup_test_container(&connection);
    let test_textfile_embedding =
      setup_test_textfile_embedding(&connection, test_container.id);
    let previous_page_option =
      test_textfile_embedding.get_previous_page(&mut connection).await.unwrap();

    assert!(
      previous_page_option.is_none(),
      "Previous page exists which should not."
    );
    // More assertions can be added based on expected data
  }

  // Additional test cases would be added here based on other implementations in types.rs
}
