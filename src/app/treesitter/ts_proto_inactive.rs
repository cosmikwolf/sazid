#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Default, Debug, Clone, PartialEq, ::prost::Message)]
pub struct SyntaxTree {
  #[prost(message, optional, tag = "1")]
  pub root: ::core::option::Option<Node>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Default, Debug, Clone, PartialEq, ::prost::Message)]
pub struct Node {
  #[prost(string, tag = "1")]
  pub r#type: ::prost::alloc::string::String,
  #[prost(message, repeated, tag = "2")]
  pub children: ::prost::alloc::vec::Vec<Node>,
  #[prost(uint32, tag = "3")]
  pub start_byte: u32,
  #[prost(uint32, tag = "4")]
  pub end_byte: u32,
  #[prost(bool, tag = "5")]
  pub is_error: bool,
  #[prost(bool, tag = "6")]
  pub has_error: bool,
  #[prost(String, optional, tag = "7")]
  pub node_name: ::prost::alloc::borrow,
}
