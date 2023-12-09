use async_openai::types::CreateEmbeddingResponse;
use std::{error::Error, str};
use tokio_postgres::{
  types::{to_sql_checked, FromSql, IsNull, ToSql, Type},
  Row, SimpleQueryMessage, SimpleQueryRow,
};
use tokio_util::bytes::{BufMut, BytesMut};

use crate::app::errors::SazidError;

#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
  pub embedding: EmbeddingVector,
  pub category: String,
  pub data: EmbeddingData,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingVector {
  data: Vec<f64>,
}

impl From<Vec<f32>> for EmbeddingVector {
  fn from(data: Vec<f32>) -> Self {
    EmbeddingVector { data: data.iter().map(|elem| *elem as f64).collect::<Vec<f64>>() }
  }
}
impl From<Vec<f64>> for EmbeddingVector {
  fn from(data: Vec<f64>) -> Self {
    EmbeddingVector { data }
  }
}

impl EmbeddingVector {
  fn len(&self) -> usize {
    self.data.len()
  }
  fn is_empty(&self) -> bool {
    self.data.is_empty()
  }
  fn iter(&self) -> std::slice::Iter<f64> {
    self.data.iter()
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingFileInfo {
  pub filename: String,
  pub md5sum: String,
}

impl Embedding {
  pub fn new(embedding: EmbeddingVector, data: EmbeddingData, category: String) -> Self {
    Embedding { embedding, data, category }
  }
  pub fn content(&self) -> &str {
    self.data.content()
  }
  pub fn len(&self) -> usize {
    self.embedding.len()
  }

  pub fn is_empty(&self) -> bool {
    self.embedding.is_empty()
  }

  pub fn string_representation(&self) -> String {
    let vec_str = self.embedding.iter().map(|elem| elem.to_string()).collect::<Vec<String>>().join(",");
    format!("'[{}]'", vec_str)
  }

  pub fn table_name(&self) -> String {
    format!("{}_embedding", self.category)
  }

  pub fn from_simple_query_messages(
    simple_query_messages: &[SimpleQueryMessage],
    category: &str,
  ) -> Result<Vec<Embedding>, SazidError> {
    simple_query_messages
      .iter()
      .filter_map(|simple_query_message| match simple_query_message {
        SimpleQueryMessage::Row(simple_query_row) => Some(Embedding::from_simple_query_row(simple_query_row, category)),
        _ => None,
      })
      .collect::<Result<Vec<Embedding>, SazidError>>()
  }

  pub fn try_from_row(row: Row, category: &str) -> Result<Embedding, SazidError> {
    let embedding = row.try_get("embedding")?;
    let data = EmbeddingData::try_from_row(row, category)?;
    Ok(Embedding::new(embedding, data, category.to_string()))
  }
  pub fn from_simple_query_row(simple_query_row: &SimpleQueryRow, category: &str) -> Result<Embedding, SazidError> {
    let embedding = simple_query_row
      .get("embedding")
      .unwrap()
      .trim_matches(|c| c == '[' || c == ']')
      .split(',')
      .map(|num_str| num_str.trim().parse())
      .collect::<Result<Vec<f64>, _>>()
      .unwrap();

    let data = match EmbeddingData::variant_from_category(category) {
      Ok(EmbeddingData::PlainTextEmbedding(_)) => {
        EmbeddingData::PlainTextEmbedding(PlainTextEmbedding::try_from(simple_query_row).unwrap())
      },
      Ok(EmbeddingData::TextFileEmbedding(_)) => {
        EmbeddingData::TextFileEmbedding(TextFileEmbeddingData::try_from(simple_query_row).unwrap())
      },
      Err(e) => return Err(e),
    };

    Ok(Embedding::new(embedding.into(), data, category.to_string()))
  }
}
impl From<CreateEmbeddingResponse> for EmbeddingVector {
  fn from(data: CreateEmbeddingResponse) -> Self {
    data
      .data
      .iter()
      .flat_map(|e| e.embedding.iter().map(|val| *val as f64).collect::<Vec<f64>>())
      .collect::<Vec<f64>>()
      .into()
  }
}
impl<'a> FromSql<'a> for EmbeddingVector {
  fn from_sql(_: &Type, raw: &'a [u8]) -> Result<EmbeddingVector, Box<dyn Error + Sync + Send>> {
    let str_data = str::from_utf8(raw)?;
    let data_strs = str_data
      .trim_matches(|c| c == '[' || c == ']')
      .split(',')
      .map(|num_str| num_str.parse())
      .collect::<Result<Vec<f64>, _>>()
      .map_err(|e| Box::new(e) as Box<dyn Error + Sync + Send>)?;
    Ok(data_strs.into())
  }

  fn accepts(ty: &Type) -> bool {
    ty.name() == "vector"
  }
}

impl ToSql for Embedding {
  to_sql_checked!();
  fn encode_format(&self, _ty: &Type) -> tokio_postgres::types::Format {
    tokio_postgres::types::Format::Text
  }
  fn to_sql(&self, _: &Type, out: &mut BytesMut) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
    let vec_str = self.embedding.iter().map(|elem| elem.to_string()).collect::<Vec<String>>().join(",");
    let vec_formatted = format!("[{}]", vec_str);
    out.put_slice(vec_formatted.as_bytes());
    Ok(IsNull::No)
  }

  fn accepts(ty: &Type) -> bool {
    ty.name() == "vector"
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EmbeddingData {
  PlainTextEmbedding(PlainTextEmbedding),
  TextFileEmbedding(TextFileEmbeddingData),
}
impl Iterator for EmbeddingData {
  type Item = EmbeddingData;
  fn next(&mut self) -> Option<Self::Item> {
    Some(self.clone())
  }
}
impl EmbeddingData {
  fn content(&self) -> &str {
    match self {
      EmbeddingData::PlainTextEmbedding(plain_text) => plain_text.content(),
      EmbeddingData::TextFileEmbedding(text_file) => text_file.content(),
    }
  }
  fn variants() -> Vec<EmbeddingData> {
    vec![
      EmbeddingData::PlainTextEmbedding(PlainTextEmbedding::default()),
      EmbeddingData::TextFileEmbedding(TextFileEmbeddingData::default()),
    ]
  }

  fn category_name(&self) -> &str {
    match self {
      EmbeddingData::PlainTextEmbedding(_) => "plain_text",
      EmbeddingData::TextFileEmbedding(_) => "text_file",
    }
  }
  pub fn new_from_row_with_category(row: Row, category: &str) -> Result<Self, SazidError> {
    match Self::variant_from_category(category) {
      Ok(EmbeddingData::PlainTextEmbedding(_)) => {
        Ok(EmbeddingData::PlainTextEmbedding(PlainTextEmbedding::try_from(row)?))
      },
      Ok(EmbeddingData::TextFileEmbedding(_)) => {
        Ok(EmbeddingData::TextFileEmbedding(TextFileEmbeddingData::try_from(row)?))
      },
      Err(e) => Err(e),
    }
  }

  pub fn new_plaintext(content: &str) -> Self {
    EmbeddingData::PlainTextEmbedding(PlainTextEmbedding { id: None, content: content.to_string() })
  }

  pub fn new_textfile(content: &str, filename: &str, checksum: &str) -> Self {
    EmbeddingData::TextFileEmbedding(TextFileEmbeddingData {
      id: None,
      content: content.to_string(),
      filename: filename.to_string(),
      checksum: checksum.to_string(),
    })
  }

  pub fn variant_from_category(category: &str) -> Result<EmbeddingData, SazidError> {
    Ok(
      Self::variants()
        .iter()
        .find(|variant| variant.category_name() == category)
        .expect("No matching category")
        .clone(),
    )
  }

  fn try_from_row(row: Row, category: &str) -> Result<Self, SazidError> {
    match Self::variant_from_category(category) {
      Ok(EmbeddingData::PlainTextEmbedding(_)) => {
        Ok(EmbeddingData::PlainTextEmbedding(PlainTextEmbedding::try_from(row)?))
      },
      Ok(EmbeddingData::TextFileEmbedding(_)) => {
        Ok(EmbeddingData::TextFileEmbedding(TextFileEmbeddingData::try_from(row)?))
      },
      Err(e) => Err(e),
    }
  }

  pub fn try_from_simple_query_row(row: &SimpleQueryRow, category: &str) -> Result<Self, SazidError> {
    match Self::variant_from_category(category) {
      Ok(EmbeddingData::PlainTextEmbedding(_)) => {
        Ok(EmbeddingData::PlainTextEmbedding(PlainTextEmbedding::try_from(row)?))
      },
      Ok(EmbeddingData::TextFileEmbedding(_)) => {
        Ok(EmbeddingData::TextFileEmbedding(TextFileEmbeddingData::try_from(row)?))
      },
      Err(e) => Err(e),
    }
  }
}

trait EmbeddingDataType: TryFrom<Row> {
  fn content(&self) -> &str;
  //fn try_from(row: &'a SimpleQueryRow) -> Result<Self, SazidError>;
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlainTextEmbedding {
  id: Option<i32>,
  content: String,
}

impl PlainTextEmbedding {
  pub fn new(content: &str) -> Self {
    PlainTextEmbedding { id: None, content: content.to_string() }
  }
}
impl EmbeddingDataType for PlainTextEmbedding {
  fn content(&self) -> &str {
    &self.content
  }
}

impl TryFrom<&SimpleQueryRow> for PlainTextEmbedding {
  type Error = SazidError;
  fn try_from(row: &SimpleQueryRow) -> Result<Self, SazidError> {
    Ok(PlainTextEmbedding {
      id: Some(row.get("id").unwrap().parse::<i32>().unwrap()),
      content: row.get("content").unwrap().into(),
    })
  }
}

impl TryFrom<Row> for PlainTextEmbedding {
  type Error = SazidError;

  fn try_from(row: Row) -> Result<Self, SazidError> {
    Ok(PlainTextEmbedding { id: Some(row.try_get("id")?), content: row.try_get("content")? })
  }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextFileEmbeddingData {
  id: Option<i32>,
  content: String,
  filename: String,
  checksum: String,
}

impl TextFileEmbeddingData {
  pub fn new(content: &str, filepath: &str) -> Self {
    let filename = filepath.split('/').last().unwrap();
    let checksum = blake3::hash(content.as_bytes()).to_hex().to_string();

    TextFileEmbeddingData { id: None, content: content.to_string(), filename: filename.to_string(), checksum }
  }
}

impl EmbeddingDataType for TextFileEmbeddingData {
  fn content(&self) -> &str {
    &self.content
  }
}

impl TryFrom<&SimpleQueryRow> for TextFileEmbeddingData {
  type Error = SazidError;
  fn try_from(row: &SimpleQueryRow) -> Result<Self, SazidError> {
    Ok(TextFileEmbeddingData {
      id: Some(row.get("id").unwrap().parse::<i32>().unwrap()),
      content: row.get("content").unwrap().into(),
      filename: row.get("filename").unwrap().into(),
      checksum: row.get("checksum").unwrap().into(),
    })
  }
}

impl TryFrom<Row> for TextFileEmbeddingData {
  type Error = SazidError;

  fn try_from(row: Row) -> Result<Self, SazidError> {
    Ok(TextFileEmbeddingData {
      id: Some(row.try_get("id")?),
      content: row.try_get("content")?,
      filename: row.try_get("filename")?,
      checksum: row.try_get("checksum")?,
    })
  }
}
