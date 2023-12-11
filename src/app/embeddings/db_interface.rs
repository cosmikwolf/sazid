use super::db_model::*;
use super::schema::plaintext_embeddings::table as PlainTextTable;
use super::schema::*;
use async_trait::async_trait;
use diesel::backend::Backend;
use diesel::prelude::*;
use diesel::query_builder::AsQuery;
use diesel::sql_types::{BigInt, SqlType};
use diesel::{helper_types::SqlTypeOf, pg::Pg};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use dotenv::dotenv;
use pgvector::{Vector, VectorExpressionMethods};
pub struct VectorDb {
  pub conn: AsyncPgConnection,
}

#[async_trait]
pub trait EmbeddingType<E, S, I, DB>
where
  E: Queryable<T, DB>,
  S: SqlType,
I: Insertable<T, DB>,
  DB: Backend,
{
  async fn add(db: &VectorDb, embedding: I) -> Result<E, Box<dyn std::error::Error>>;

  async fn get_by_id(db: &VectorDb, id: i64) -> Result<E, Box<dyn std::error::Error>>;

  async fn find_similar(db: &VectorDb, embedding: Vector) -> Result<Vec<E>, Box<dyn std::error::Error>>;

  // Define other methods as needed...
}
type a =  <plaintext_embeddings::table as Table>::AllColumns as Expression>::SqlType;

#[async_trait]
impl<
    PlainTextEmbedding: diesel::Queryable<
    <<plaintext_embeddings::table as Table>::AllColumns as Expression>::SqlType >
    , diesel::pg::Pg> + std::marker::Send,
    PlainTextTable: diesel::QuerySource + diesel::sql_types::SingleValue,
    InsertablePlainTextEmbedding,
    Pg,
  > EmbeddingType<PlainTextEmbedding, PlainTextTable, InsertablePlainTextEmbedding, Pg> for PlainTextEmbedding
{
  async fn add(
    db: &VectorDb,
    embedding: InsertablePlainTextEmbedding,
  ) -> Result<PlainTextEmbedding, Box<dyn std::error::Error>> {
    Ok(
      diesel::insert_into(plaintext_embeddings::table)
        .values(embedding)
        .returning(plaintext_embeddings::id)
        .get_result(db.conn)
        .await?,
    )
  }
  fn get_by_id(db: VectorDb, id: i64) -> Result<PlainTextEmbedding, Box<dyn std::error::Error>> {
    let embedding = plaintext_embeddings::table.find(id).first::<PlainTextEmbedding>(&mut self.conn).await?;
    Ok(embedding)
  }
  fn find_similar(embedding: Vector) -> Result<Vec<T>, Box<dyn std::error::Error>> {
    let query = plaintext_embeddings::table
      .select(plaintext_embeddings::all_columns)
      .order(plaintext_embeddings::embedding.cosine_distance(embedding))
      .limit(10);
    let embeddings = query.load::<PlainTextEmbedding>(&mut self.conn).await?;
    Ok(embeddings)
  }
}

impl VectorDb {
  pub async fn init() -> Result<Self, Box<dyn std::error::Error>> {
    dotenv().ok();
    let database_url = std::env::var("DATABASE_URL")?;
    Ok(VectorDb { conn: AsyncPgConnection::establish(&database_url).await? })
  }

  pub async fn add_plaintext_embedding(
    &mut self,
    content: &str,
    embedding: &Vector,
  ) -> Result<i64, Box<dyn std::error::Error>> {
    let new_embedding = NewPlainTextEmbedding { content: content.to_string(), embedding: embedding.clone() };

    Ok(
      diesel::insert_into(plaintext_embeddings::table)
        .values(&new_embedding)
        .returning(plaintext_embeddings::id)
        .get_result(&mut self.conn)
        .await?,
    )
  }

  pub async fn get_plaintext_embedding_by_id(
    &mut self,
    id: i64,
  ) -> Result<PlainTextEmbedding, Box<dyn std::error::Error>> {
    let embedding = plaintext_embeddings::table.find(id).first::<PlainTextEmbedding>(&mut self.conn).await?;
    Ok(embedding)
  }

  pub async fn get_similar_plaintext_embeddings(
    &mut self,
    embedding: &Vector,
  ) -> Result<Vec<PlainTextEmbedding>, Box<dyn std::error::Error>> {
    let query = plaintext_embeddings::table
      .select(plaintext_embeddings::all_columns)
      .order(plaintext_embeddings::embedding.cosine_distance(embedding))
      .limit(10);
    let embeddings = query.load::<PlainTextEmbedding>(&mut self.conn).await?;
    Ok(embeddings)
  }

  pub async fn add_textfile_embedding(
    &mut self,
    content: &str,
    embedding: &Vector,
    filename: &str,
    checksum: &str,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let new_embedding = NewTextFileEmbedding {
      content: content.to_string(),
      filename: filename.to_string(),
      checksum: checksum.to_string(),
      embedding: embedding.clone(),
    };
    diesel::insert_into(textfile_embeddings::table).values(&new_embedding).execute(&mut self.conn).await?;
    Ok(())
  }

  pub async fn get_textfile_embedding_by_id(
    &mut self,
    id: i64,
  ) -> Result<TextFileEmbedding, Box<dyn std::error::Error>> {
    let embedding = textfile_embeddings::table.find(id).first::<TextFileEmbedding>(&mut self.conn).await?;
    Ok(embedding)
  }

  pub async fn get_similar_textfile_embeddings(
    &mut self,
    embedding: &Vector,
  ) -> Result<Vec<TextFileEmbedding>, Box<dyn std::error::Error>> {
    let query = textfile_embeddings::table
      .select(textfile_embeddings::all_columns)
      .order(textfile_embeddings::embedding.cosine_distance(embedding))
      .limit(10);
    let embeddings = query.load::<TextFileEmbedding>(&mut self.conn).await?;
    Ok(embeddings)
  }
}
