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
  // use diesel::{backend::Backend, serialize::ToSql, sql_types};
  use diesel::expression::{AsExpression, Expression};
  use diesel::pg::PgConnection;
  use diesel::{Connection, QueryDsl, RunQueryDsl};
  use dotenv::dotenv;
  use pgvector::{Vector, VectorExpressionMethods};
  use std::env;

  table! {
      use diesel::sql_types::*;
       use pgvector::sql_types::*;

      plaintext_embeddings (id) {
          id -> Int4,
          // content -> Text,
          embedding -> Nullable<Vector>,
      }
  }
  use plaintext_embeddings as plaintext;

  #[derive(Queryable, Selectable)]
  #[diesel(table_name = plaintext)]
  pub struct PlainTextEmbedding {
    id: i32,
    // content: String,
    embedding: Option<pgvector::Vector>,
  }

  #[derive(Insertable)]
  #[diesel(table_name = plaintext)]
  pub struct NewPlainTextEmbedding {
    // pub content: String,
    pub embedding: Option<pgvector::Vector>,
  }

  #[test]
  fn test_diesel() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let mut conn =
      PgConnection::establish(&database_url).unwrap_or_else(|_| panic!("Error connecting to {}", database_url));

    diesel::sql_query("CREATE EXTENSION IF NOT EXISTS vectors").execute(&mut conn)?;
    println!("created extension");
    diesel::sql_query("DROP TABLE IF EXISTS diesel_items").execute(&mut conn)?;
    diesel::sql_query("CREATE TABLE diesel_items (id serial PRIMARY KEY, embedding vector(3))").execute(&mut conn)?;
    // create an async connection
    // let mut connection = AsyncPgConnection::establish(&std::env::var("DATABASE_URL")?).await?;
    let new_items = vec![
      NewPlainTextEmbedding { embedding: Some(Vector::from(vec![1.0, 1.0, 2.0])) },
      NewPlainTextEmbedding { embedding: Some(Vector::from(vec![1.0, 2.0, 1.0])) },
      NewPlainTextEmbedding { embedding: Some(Vector::from(vec![1.0, 2.0, 2.0])) },
      NewPlainTextEmbedding { embedding: None },
    ];
    // let new_item = NewPlainTextEmbedding { content: "hello world".to_string(), embedding: Some(embedding) };

    diesel::insert_into(plaintext::table)
      .values(&new_items)
      // .returning(PlainTextEmbedding::as_returning())
      .get_result::<PlainTextEmbedding>(&mut conn)
      .expect("Error saving new post");

    // diesel::insert_into(plaintext_embeddings::table)
    //   .values(&new_item)
    //   .get_result::<PlainTextEmbedding>(&mut connection)
    //     .expect("Error saving new post")
    //    .await?;

    // use ordinary diesel query dsl to construct your query
    let one = plaintext::table
      .filter(plaintext::id.gt(0))
      .select(PlainTextEmbedding::as_select())
      .load::<PlainTextEmbedding>(&mut conn)?;
    assert_eq!(1, one.len());

    let all = plaintext::table.load::<PlainTextEmbedding>(&mut conn)?;
    assert_eq!(4, all.len());

    let neighbors = plaintext::table
      .order(plaintext::embedding.cosine_distance(Vector::from(vec![1.0, 1.0, 1.0])))
      .limit(5)
      .load::<PlainTextEmbedding>(&mut conn)?;

    assert_eq!(vec![1, 2, 3, 4], neighbors.iter().map(|v| v.id).collect::<Vec<i32>>());

    // execute the query via the provided
    // async `diesel_async::RunQueryDsl`
    Ok(())
  }
}
