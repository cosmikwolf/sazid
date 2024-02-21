#[cfg(test)]
mod tests {
  use sazid::app::functions::treesitter_function::treesitter_query;
  use sazid::app::session_config::SessionConfig;
  use sazid::app::treesitter::treesitter_query::TreeData;
  use sazid::app::treesitter::*;
  use std::path::PathBuf;
  use std::{fs::File, io::Write};
  use tempfile::tempdir;
  use tempfile::TempDir;
  use tree_sitter::Parser;

  fn create_tempdir() -> TempDir {
    tempdir().unwrap()
  }

  fn create_temp_file(dir: &TempDir, contents: &str) -> PathBuf {
    println!("dir: {:?}", dir);
    let file_path = dir.path().join("test.rs");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(contents.as_bytes()).unwrap();
    file_path
  }

  fn create_temp_file_with_lines(lines: Vec<&str>) -> PathBuf {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(lines.join("\n").as_bytes()).unwrap();
    file_path
  }

  fn create_session_config_with_tempdir_accessor(
    dir: &TempDir,
  ) -> SessionConfig {
    SessionConfig {
      accessible_paths: vec![dir.path().to_path_buf()],
      ..Default::default()
    }
  }

  fn setup_parse(source_code: &str) -> Result<TreeData, anyhow::Error> {
    let dir = create_tempdir();
    let path = create_temp_file(&dir, source_code);
    let mut parser = Parser::new();
    let language = tree_sitter_rust::language();
    let timeout = 20000;
    parser.set_language(language).expect("Error loading Rust grammar");
    TreeData::new(&mut parser, path, language)
  }

  #[test]
  fn test_parse_file_find_node() {
    let source_code = r#"
        //comment 1
        fn main() {
          let a = 2;
          println!("Hello, world! {} {}", a, 2);
        }
    /*
    comment 2
    */
        fn test(arg: &str) -> Result<bool> {
            if arg.len() > 0 {
                true
            } else {
                false
            }
        }
    "#;
    let parse_result = setup_parse(source_code);
    assert!(parse_result.is_ok());
    let tree_data = parse_result.unwrap();
    let tree_protobuf = tree_data.to_protobuf();
    let tags = tree_data.query_tags();
    let comments = tree_data.query_comments();
    // println!("tree:\nj{:#?}", tree_protobuf);
    // println!("tree:\n{}", tree_data);
    // println!("tree protobuf:\n{:#?}", tree_protobuf);
    // println!(
    //   "declarations: \n{:#?}",
    //   declarations
    //     .iter()
    //     .map(|n| { node_to_protobuf(*n, 1, false, Some(&tree_data)) })
    //     .collect::<Vec<ts_proto::Node>>()
    // );
    println!(
      "tags: \n{:#?}",
      tags
        .iter()
        .flat_map(|n| { tree_data.node_to_protobuf(*n,) })
        .collect::<Vec<ts_proto::Node>>()
    );
    // println!("tags pb: \n{:#?}",);
    // println!(
    //   "comments: \n{:#?}",
    //   comments.iter().flat_map(|n| { tree_data.node_to_protobuf(*n,) }).collect::<Vec<ts_proto::Node>>()
    // );
    // tree_data.get_tags();
    // println!(
    //   "functions: \n{:#?}",
    //   function_nodes
    //     .iter()
    //     .map(|n| { node_to_protobuf(*n, 0, false, Some(&tree_data)) })
    //     .collect::<Vec<ts_proto::Node>>()
    // );
    // assert_eq!(format!("{:#?}", tree_protobuf), "test");
    panic!()
  }

  #[test]
  fn test_file_does_not_exist() {
    let result = treesitter_query(
      vec![PathBuf::from("./tests/test_files/test_does_not_exist.rs")],
      "",
    );
    println!("{:?}", result);
    assert!(result.is_err());
    assert_eq!(
      result.unwrap_err().to_string(),
      "Error: No such file or directory (os error 2)  Path: ./tests/test_files/test_does_not_exist.rs"
    );
  }

  #[test]
  fn test_treesitter_query_noresults() {
    let dir = create_tempdir();
    let source_code = r#"
        // testing testing 1 2 3
        fn main() {
          println!("Hello, world!");
        }
    "#;
    let path = create_temp_file(&dir, source_code);
    let ts_query = "";
    let result = treesitter_query(vec![path], ts_query);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some("no matches found".to_string()));
  }

  #[test]
  fn test_treesitter_query_success() {
    let dir = create_tempdir();
    let source_code = r#"
        fn main() {
          println!("Hello, world!");
        }
    "#;
    let path = create_temp_file(&dir, source_code);
    let ts_query = "(function_item) @function";
    let result = treesitter_query(vec![path], ts_query);
    println!("{:?}", result);
    assert!(result.is_ok());
    assert_eq!(
      result.unwrap().expect("result should be something").to_string(),
      "fn main() {\n          println!(\"Hello, world!\");\n        }"
    );
  }
}
