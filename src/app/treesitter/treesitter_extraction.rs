// full_code_extraction_iterations.rs
// Comprehensive Rust code for Tree-sitter Code Extraction Iterations 1-4

use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use tree_sitter::Parser;
use tree_sitter::Query;
use tree_sitter::QueryCursor;

pub fn extract_parsed_nodes(source_code: &str) {
  let mut parser = Parser::new();
  let language = tree_sitter_rust::language();
  parser.set_language(language).expect("Error loading Rust grammar");
  let tree = parser.parse(source_code, None).unwrap();
  let root_node = tree.root_node();
  println!("Root node: {:#?}", root_node);

  println!("{}", tree.root_node().to_sexp())
}

pub fn extract_comments(source_code: &str) -> Value {
  let mut parser = Parser::new();
  let language = tree_sitter_rust::language();
  parser.set_language(language).expect("Error loading Rust grammar");

  let parsed = parser.parse(source_code, None).expect("Error parsing source code");
  let root_node = parsed.root_node();
  let mut comments = Vec::new();
  let comment_query = tree_sitter::Query::new(language, "(line_comment) @comment (block_comment) @comment").unwrap();
  let mut query_cursor = tree_sitter::QueryCursor::new();
  let query_matches = query_cursor.matches(&comment_query, root_node, source_code.as_bytes());

  for match_ in query_matches {
    for capture in match_.captures {
      if capture.node.kind() == "line_comment" || capture.node.kind() == "block_comment" {
        let comment_range = capture.node.range();
        let comment = &source_code[comment_range.start_byte..comment_range.end_byte];
        comments.push(comment.to_string());
      }
    }
  }

  let comments: Vec<Value> = comments.iter().map(|c| json!({"type": "comment", "text": c})).collect();
  json!({"source": source_code, "constructs": comments})
}

pub fn extract_declarations(source_code: &str) -> Value {
  let mut parser = Parser::new();
  let language = tree_sitter_rust::language();
  parser.set_language(language).expect("Error loading Rust grammar");

  let parsed = parser.parse(source_code, None).expect("Error parsing source code");
  let root_node = parsed.root_node();

  let mut declarations = HashMap::new();
  let declarations_query = tree_sitter::Query::new(
    language,
    "
    (function_item) @function
        (struct_item) @struct
        (enum_item) @enum
        (trait_item) @trait
    ",
  )
  .unwrap();

  let mut query_cursor = tree_sitter::QueryCursor::new();
  let query_matches = query_cursor.matches(&declarations_query, root_node, source_code.as_bytes());

  for match_ in query_matches {
    for capture in match_.captures {
      let capture_text = &source_code[capture.node.range().start_byte..capture.node.range().end_byte].to_string();
      match capture.node.kind() {
        "function_item" => {
          declarations.entry("functions").or_insert_with(Vec::new).push(capture_text.clone());
        },
        "struct_item" => {
          declarations.entry("structs").or_insert_with(Vec::new).push(capture_text.clone());
        },
        "enum_item" => {
          declarations.entry("enums").or_insert_with(Vec::new).push(capture_text.clone());
        },
        "trait_item" => {
          declarations.entry("traits").or_insert_with(Vec::new).push(capture_text.clone());
        },
        _ => {},
      }
    }
  }

  let declarations_list: Vec<Value> = declarations
    .iter()
    .flat_map(|(type_name, items)| {
      items.iter().map(|item| json!({"type": type_name, "text": item})).collect::<Vec<Value>>()
    })
    .collect();
  json!({"source": source_code, "constructs": declarations_list})
}

pub fn extract_code_constructs(source_code: &str) -> Value {
  let mut parser = Parser::new();
  let language = tree_sitter_rust::language();
  parser.set_language(language).expect("Error loading Rust grammar");

  let parsed = parser.parse(source_code, None).expect("Error parsing source code");
  let root_node = parsed.root_node();

  let complex_query = Query::new(
    language,
    "
        (line_comment) @comment
        (block_comment) @comment
        (impl_item) @implementation
        (function_item) @function
        (if_expression) @if
        (match_expression) @match
        (_literal) @literal
        (attribute_item) @attribute
        (macro_invocation) @macro
    ",
  )
  .expect("Error creating query");

  let mut query_cursor = QueryCursor::new();
  let query_matches = query_cursor.matches(&complex_query, root_node, source_code.as_bytes());

  let mut constructs = json!({
      "implementations": [],
      "definitions": [],
      "if_expressions": [],
      "match_expressions": [],
      "literals": [],
      "attributes": [],
      "macros": []
  });

  for match_ in query_matches {
    for capture in match_.captures {
      let capture_text = source_code[capture.node.range().start_byte..capture.node.range().end_byte].to_string();
      let construct_type = match capture.node.kind() {
        "impl_item" => "implementations",
        "function_item" => "definitions",
        "if_expression" => "if_expressions",
        "match_expression" => "match_expressions",
        "_literal" => "literals",
        "attribute_item" => "attributes",
        "macro_invocation" => "macros",
        _ => "",
      };

      if !construct_type.is_empty() {
        constructs[construct_type].as_array_mut().unwrap().push(json!(capture_text));
      }
    }
  }

  let constructs_list: Vec<Value> = constructs
    .as_object()
    .unwrap()
    .iter()
    .flat_map(|(type_name, items)| {
      items.as_array().unwrap().iter().map(|item| json!({"type": type_name, "text": item})).collect::<Vec<Value>>()
    })
    .collect();

  json!({"source": source_code, "constructs": constructs_list})
}

pub fn convert_to_vector_representation(code_constructs: &Value) -> Vec<f64> {
  // This function should take the code constructs and convert them into a form suitable for machine learning.
  // For example purposes, we are going to represent each code construct as a fixed-size vector of f64.
  // In a real scenario, this would involve natural language processing and other techniques.
  println!("{:#?}", code_constructs);

  // Placeholder for the vector representations of the code constructs
  let mut vectors: Vec<Vec<f64>> = Vec::new();

  if let Some(constructs_array) = code_constructs.as_array() {
    for construct in constructs_array {
      // Placeholder: Convert each construct to a fixed-size vector representation
      let construct_vector = vec![0.0_f64; 300]; // Example fixed-size vector
      vectors.push(construct_vector);
    }
  }

  // Flatten the vector of vectors into a single vector for simplicity in this example
  vectors.into_iter().flatten().collect()
}
