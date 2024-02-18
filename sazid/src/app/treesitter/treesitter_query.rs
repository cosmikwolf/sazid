use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::str::Utf8Error;
use std::string::FromUtf8Error;

use super::ts_proto;
use anyhow::{anyhow, Context, Result};
use tree_sitter::InputEdit;
use tree_sitter::Language;
use tree_sitter::Node;
use tree_sitter::Parser;
use tree_sitter::Point;
use tree_sitter::Query;
use tree_sitter::QueryCursor;
use tree_sitter::Tree;

const RUST_COMMENT_QUERY: &str = "
(line_comment) @comment
(block_comment) @comment
";

pub struct TreeData {
  pub(crate) source_code: Vec<u8>,
  pub(crate) tree: Tree,
  path: PathBuf,
  edits: Option<Vec<Edit>>,
}

#[derive(Debug, Clone)]
pub struct Edit {
  pub position: usize,
  pub deleted_length: usize,
  pub inserted_text: Vec<u8>,
}

impl fmt::Display for TreeData {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.tree.root_node().to_sexp())
  }
}

impl TreeData {
  pub fn new(
    parser: &mut Parser,
    path: PathBuf,
    language: Language,
  ) -> Result<Self, anyhow::Error> {
    parser.set_language(language)?;
    let source_code = fs::read(path.clone()).expect("Error reading file");
    let tree = parser.parse(&source_code, None).unwrap();
    Ok(TreeData { tree, source_code, path, edits: None })
  }

  pub fn node_src(&self, node: Node) -> Result<&str, Utf8Error> {
    let start = node.start_byte();
    let end = node.end_byte();
    let src = &self.source_code[start..end];
    std::str::from_utf8(src)
  }

  pub fn execute_edits(&mut self, parser: &mut Parser) -> Result<String> {
    let mut output = String::new();
    if let Some(edits) = &self.edits.clone() {
      output.push_str(&format!(
        "BEFORE:\n{}",
        String::from_utf8_lossy(&self.source_code)
      ));
      edits.iter().for_each(|edit| {
        // let edit = parse_edit_flag(self.source_code, edit)?
        self.format_edit(edit).unwrap();
        self.tree = parser.parse(&self.source_code, Some(&self.tree)).unwrap();
      });
      output.push_str(&format!(
        "AFTER:\n{}",
        String::from_utf8_lossy(&self.source_code)
      ));
      Ok(output)
    } else {
      Ok("no edits made".to_string())
    }
  }

  pub fn get_tree_node_errors(&self) -> String {
    let mut output = String::new();
    let mut cursor = self.tree.walk();
    let mut first_error = None;
    loop {
      let node = cursor.node();
      if node.has_error() {
        if node.is_error() || node.is_missing() {
          first_error = Some(node);
          break;
        } else if !cursor.goto_first_child() {
          break;
        }
      } else if !cursor.goto_next_sibling() {
        break;
      }
    }

    match first_error {
      None => "No errors found\n".to_string(),
      Some(_) => {
        output.push_str(self.path.to_str().unwrap());
        if let Some(node) = first_error {
          let start = node.start_position();
          let end = node.end_position();
          output.push_str("\t(");
          if node.is_missing() {
            if node.is_named() {
              output.push_str(&format!("MISSING {}", node.kind()));
            } else {
              output.push_str(&format!(
                "MISSING \"{}\"",
                node.kind().replace('\n', "\\n")
              ));
            }
          } else {
            output.push_str(node.kind());
          }
          output.push_str(&format!(
            " [{}, {}] - [{}, {}])",
            start.row, start.column, end.row, end.column
          ));
        }
        output.push('\n');
        output
      },
    }
  }

  pub fn format_edit(&mut self, edit: &Edit) -> Result<InputEdit> {
    let start_byte = edit.position;
    let old_end_byte = edit.position + edit.deleted_length;
    let new_end_byte = edit.position + edit.inserted_text.len();
    let start_position =
      position_for_offset(self.source_code.as_slice(), start_byte)?;
    let old_end_position =
      position_for_offset(self.source_code.as_slice(), old_end_byte)?;
    self
      .source_code
      .splice(start_byte..old_end_byte, edit.inserted_text.iter().cloned());
    let new_end_position =
      position_for_offset(self.source_code.as_slice(), new_end_byte)?;
    let edit = InputEdit {
      start_byte,
      old_end_byte,
      new_end_byte,
      start_position,
      old_end_position,
      new_end_position,
    };
    self.tree.edit(&edit);
    Ok(edit)
  }

  pub fn query_comments(&self) -> Vec<Node> {
    let query = Query::new(self.tree.language(), RUST_COMMENT_QUERY).unwrap();
    let mut cursor = QueryCursor::new();
    cursor
      .matches(
        //
        &query,
        self.tree.root_node(),
        self.source_code.as_slice(),
      )
      .map(|m| m.captures[0].node)
      .collect()
  }

  pub fn query_tags(&self) -> Vec<Node> {
    let query = Query::new(
      self.tree.language(),
      &format!("{}\n{}", tree_sitter_rust::TAGGING_QUERY, RUST_COMMENT_QUERY),
    )
    .unwrap();
    let mut cursor = QueryCursor::new();
    cursor
      .matches(
        //
        &query,
        self.tree.root_node(),
        self.source_code.as_slice(),
      )
      .map(|m| {
        println!("m: {:#?}", m);
        m.captures[0].node
      })
      .collect()
  }

  pub fn to_protobuf(&self) -> Result<ts_proto::SyntaxTree, FromUtf8Error> {
    Ok(ts_proto::SyntaxTree {
      root: Some(self.node_to_protobuf(self.tree.root_node()).unwrap()),
    })
  }

  pub fn node_to_protobuf(
    &self,
    node: Node,
  ) -> Result<ts_proto::Node, FromUtf8Error> {
    let source_file = Some(ts_proto::SourceFile {
      path: self.path.to_str().unwrap().to_string(),
    });
    Ok(ts_proto::Node {
      id: node.id() as u32,
      r#type: node.kind().to_string(),
      tag_identifier: self.load_node_tag_identifier(node).unwrap(),
      start_byte: node.start_byte() as u32,
      end_byte: node.end_byte() as u32,
      is_error: node.is_error(),
      has_error: node.has_error(),
      child_count: node.child_count() as u32,
      source_file,
    })
  }

  pub fn load_node_tag_identifier(
    &self,
    node: Node,
  ) -> Result<Option<String>, FromUtf8Error> {
    let query =
      Query::new(self.tree.language(), tree_sitter_rust::TAGGING_QUERY)
        .unwrap();
    let mut cursor = QueryCursor::new();
    let matches = cursor
      .matches(
        //
        &query,
        node,
        self.source_code.as_slice(),
      )
      .next();

    match matches {
      Some(matches) => {
        let tag_node = matches.captures[1].node;
        let tag_identifier = self.node_src(tag_node).unwrap();
        Ok(Some(tag_identifier.into()))
      },
      None => Ok(None),
    }
  }

  pub fn query_tree(&self, query: &str) -> Vec<Node> {
    let query = Query::new(self.tree.language(), query).unwrap();
    let mut cursor = QueryCursor::new();
    cursor
      .matches(&query, self.tree.root_node(), self.source_code.as_slice())
      .flat_map(|m| m.captures.iter().map(|c| c.node).collect::<Vec<Node>>())
      .collect()
  }
}

pub fn position_for_offset(input: &[u8], offset: usize) -> Result<Point> {
  if offset > input.len() {
    return Err(anyhow!("Failed to address an offset: {offset}"));
  }
  let mut result = Point { row: 0, column: 0 };
  let mut last = 0;
  for pos in memchr::memchr_iter(b'\n', &input[..offset]) {
    result.row += 1;
    last = pos;
  }
  result.column = if result.row > 0 { offset - last - 1 } else { offset };
  Ok(result)
}
