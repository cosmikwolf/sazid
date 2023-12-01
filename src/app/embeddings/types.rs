use std::{error::Error, str};
use tokio_postgres::types::{to_sql_checked, FromSql, IsNull, ToSql, Type};
use tokio_util::bytes::{Buf, BufMut, BytesMut};

#[derive(Debug, Clone, PartialEq)]
pub struct Vector {
  pub data: Vec<f64>,
}

impl Vector {
  pub fn new(data: Vec<f64>) -> Self {
    Vector { data }
  }
}

impl<'a> FromSql<'a> for Vector {
  fn from_sql(_: &Type, raw: &'a [u8]) -> Result<Vector, Box<dyn Error + Sync + Send>> {
    let str_data = str::from_utf8(raw)?;
    let data_strs = str_data
      .trim_matches(|c| c == '[' || c == ']')
      .split(',')
      .map(|num_str| num_str.parse())
      .collect::<Result<Vec<f64>, _>>()?;
    Ok(Vector { data: data_strs })
  }

  fn accepts(ty: &Type) -> bool {
    ty.name() == "vector"
  }
}

impl ToSql for Vector {
  to_sql_checked!();

  fn to_sql(&self, _: &Type, out: &mut BytesMut) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
    let vec_str = self.data.iter().map(|elem| elem.to_string()).collect::<Vec<String>>().join(",");
    let vec_formatted = format!("{{{}}}", vec_str);
    out.put_slice(vec_formatted.as_bytes());
    Ok(IsNull::No)
  }

  fn accepts(ty: &Type) -> bool {
    ty.name() == "vector"
  }
}
