use std::{error::Error, str};
use tokio_postgres::{
  types::{to_sql_checked, FromSql, IsNull, ToSql, Type},
  SimpleQueryMessage, SimpleQueryRow,
};
use tokio_util::bytes::{BufMut, BytesMut};

#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingVector {
  pub embedding: Vec<f64>,
  pub data: String,
}

impl EmbeddingVector {
  pub fn new(embedding: Vec<f64>, data: String) -> Self {
    EmbeddingVector { embedding, data }
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

  pub fn from_simple_query_messages(
    simple_query_messages: &[SimpleQueryMessage],
  ) -> Result<Vec<EmbeddingVector>, std::io::Error> {
    println!("simple_query_messages: {:#?}", simple_query_messages);
    simple_query_messages
      .iter()
      .filter_map(|simple_query_message| match simple_query_message {
        SimpleQueryMessage::Row(simple_query_row) => Some(EmbeddingVector::from_simple_query_row(simple_query_row)),
        _ => None,
      })
      .collect::<Result<Vec<EmbeddingVector>, std::io::Error>>()
  }

  pub fn from_simple_query_row(simple_query_row: &SimpleQueryRow) -> Result<EmbeddingVector, std::io::Error> {
    let text = simple_query_row.get("text").unwrap();
    let embedding = simple_query_row
      .get("embedding")
      .unwrap()
      .trim_matches(|c| c == '[' || c == ']')
      .split(',')
      .map(|num_str| num_str.trim().parse())
      .collect::<Result<Vec<f64>, _>>()
      .unwrap();

    Ok(EmbeddingVector::new(embedding, text.to_string()))
  }
}

impl<'a> FromSql<'a> for EmbeddingVector {
  fn from_sql(_: &Type, raw: &'a [u8]) -> Result<EmbeddingVector, Box<dyn Error + Sync + Send>> {
    let str_data = str::from_utf8(raw)?;
    let data_strs = str_data
      .trim_matches(|c| c == '[' || c == ']')
      .split(',')
      .map(|num_str| num_str.parse())
      .collect::<Result<Vec<f64>, _>>()?;
    Ok(EmbeddingVector { embedding: data_strs, data: String::new() })
  }

  fn accepts(ty: &Type) -> bool {
    ty.name() == "vector"
  }
}

impl ToSql for EmbeddingVector {
  to_sql_checked!();
  fn encode_format(&self, _ty: &Type) -> tokio_postgres::types::Format {
    tokio_postgres::types::Format::Text
  }
  fn to_sql(&self, _: &Type, out: &mut BytesMut) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
    let vec_str = self.embedding.iter().map(|elem| elem.to_string()).collect::<Vec<String>>().join(",");
    let vec_formatted = format!("[{}]", vec_str);
    println!("vec_formatted: {:?}", vec_formatted);
    out.put_slice(vec_formatted.as_bytes());
    Ok(IsNull::No)
  }

  fn accepts(ty: &Type) -> bool {
    ty.name() == "vector"
  }
}
