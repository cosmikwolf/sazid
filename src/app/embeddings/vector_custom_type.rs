use bytes::{BytesMut, BufMut};
use std::error::Error;
use tokio_postgres::types::{FromSql, ToSql, IsNull, Type, to_sql_checked};
use std::str;

#[derive(Debug)]
pub struct Vector {
    data: Vec<f64>,
}

impl Vector {
    pub fn new(data: Vec<f64>) -> Self {
        Vector { data }
    }
}

impl<'a> FromSql<'a> for Vector {
    fn from_sql(_: &Type, raw: &'a [u8]) -> Result<Vector, Box<dyn Error + Sync + Send>> {
        let str_data = str::from_utf8(raw)?;
        let data_strs = str_data.trim_matches(|c| c == '[' || c == ']').split(',').collect::<Vec<_>>();
        let data = data_strs.into_iter().map(|s| s.parse::<f64>()).collect::<Result<Vec<_>, _>>()?;
        Ok(Vector::new(data))
    }

    fn accepts(ty: &Type) -> bool {
        match ty.name() {
            "_float8" => true,
            _ => false,
        }
    }
}

impl ToSql for Vector {
    to_sql_checked!();

    fn to_sql(&self, _: &Type, out: &mut BytesMut) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let str_data = self.data.iter().map(|num| num.to_string()).collect::<Vec<_>>().join(",");
        let bytes = str_data.as_bytes();
        out.extend_from_slice(bytes);
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        match ty.name() {
            "_float8" => true,
            _ => false,
        }
    }
j
