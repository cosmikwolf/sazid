pub mod treesitter_extraction;
pub mod treesitter_parser;
pub mod treesitter_query;

pub mod ts_proto {
  include!(concat!(env!("OUT_DIR"), "/treesitter.ts_proto.rs"));
}
