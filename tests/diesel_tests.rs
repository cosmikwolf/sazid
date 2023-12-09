// use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
// pub mod sql_types {
//   use diesel::expression::AsExpression;
//
//   #[derive(Clone, PartialEq, diesel::query_builder::QueryId, diesel::sql_types::SqlType, Debug)]
//   #[diesel(sql_type = pgvector::sql_types::Vector)]
//   #[derive(AsExpression)]
//   #[diesel(postgres_type(name = "vector"))]
//   pub struct Vector;
// }
// Helper function to set up the test database connection
#[cfg(test)]

mod vector_db_tests {
  use diesel::prelude::*;
  use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
  use dotenv::dotenv;
  use pgvector::{Vector, VectorExpressionMethods};

  table! {
      use diesel::sql_types::*;
       use pgvector::sql_types::*;

      plaintext_embeddings (id) {
          id -> BigInt,
          content -> Text,
          embedding -> Nullable<Vector>,
      }
  }
  use plaintext_embeddings as plaintext;

  #[derive(Queryable, Selectable)]
  #[diesel(table_name = plaintext)]
  pub struct PlainTextEmbedding {
    id: i64,
    content: String,
    embedding: Option<pgvector::Vector>,
  }

  #[derive(Insertable)]
  #[diesel(table_name = plaintext)]
  pub struct NewPlainTextEmbedding {
    pub content: String,
    pub embedding: Option<pgvector::Vector>,
  }

  #[tokio::test]
  async fn test_diesel() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let mut conn =
      AsyncPgConnection::establish(&std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL must be set")).await?;

    diesel::sql_query("CREATE EXTENSION IF NOT EXISTS vector").execute(&mut conn).await?;
    diesel::sql_query("DROP TABLE IF EXISTS plaintext_embeddings").execute(&mut conn).await?;
    diesel::sql_query(
      "CREATE TABLE plaintext_embeddings (id BigSerial PRIMARY KEY, content TEXT, embedding vector(3))",
    )
    .execute(&mut conn)
    .await?;
    // create an async connection
    let new_items = vec![
      NewPlainTextEmbedding { content: "hello world".to_string(), embedding: Some(Vector::from(vec![1.0, 1.0, 1.0])) },
      NewPlainTextEmbedding { content: "hello world".to_string(), embedding: Some(Vector::from(vec![1.0, 2.0, 1.0])) },
      NewPlainTextEmbedding { content: "hello world".to_string(), embedding: Some(Vector::from(vec![2.0, 1.0, 1.0])) },
      NewPlainTextEmbedding { content: "hello world".to_string(), embedding: None },
    ];

    diesel::insert_into(plaintext::table)
      .values(&new_items)
      // .returning(PlainTextEmbedding::as_returning())
      .get_result::<PlainTextEmbedding>(&mut conn).await?;

    // use ordinary diesel query dsl to construct your query
    let one = plaintext::table
      .filter(plaintext::id.eq(1))
      .select(PlainTextEmbedding::as_select())
      .load::<PlainTextEmbedding>(&mut conn)
      .await?;

    assert_eq!(1, one.len());

    let all = plaintext::table.load::<PlainTextEmbedding>(&mut conn).await?;
    assert_eq!(4, all.len());

    let neighbors = plaintext::table
      .order(plaintext::embedding.cosine_distance(Vector::from(vec![1.0, 1.0, 1.0])))
      .limit(5)
      .load::<PlainTextEmbedding>(&mut conn)
      .await?;

    assert_eq!(vec![1, 2, 3, 4], neighbors.iter().map(|v| v.id).collect::<Vec<i64>>());

    // execute the query via the provided
    // async `diesel_async::RunQueryDsl`
    Ok(())
  }
}
