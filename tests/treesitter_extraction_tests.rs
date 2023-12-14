#[cfg(test)]
mod treesitter_extraction_tests {
  use sazid::app::embeddings::treesitter_extraction::*;
  use serde_json::json;

  #[test]
  fn test_extract_comments() {
    let source_code = "// This is a comment\n// And another one\nfn main() {}";
    let result = extract_comments(source_code);
    assert_eq!(result["constructs"].as_array().unwrap().len(), 2);
  }

  #[test]
  fn test_extract_declarations() {
    let source_code =
      "struct MyStruct {}\nimpl MyStruct {\n    fn new() -> Self {\n        MyStruct {}\n    }\n}\nfn my_function() {}";
    let result = extract_declarations(source_code);
    println!("{:#?}", result);
    assert_eq!(result["constructs"].as_array().unwrap().len(), 3);
  }

  #[test]
  fn test_extract_code_constructs() {
    let source_code = "impl MyStruct {\n    fn new() -> Self {\n        MyStruct {}\n    }\n}\nlet x = if true {\n    5\n} else {\n    10\n};";
    let result = extract_code_constructs(source_code);
    assert!(result["constructs"].as_array().unwrap().len() > 0);
  }

  #[test]
  fn test_convert_to_vector_representation() {
    let construct = json!({"type": "function", "text": "fn test() {}"});
    let result = convert_to_vector_representation(&construct);
    println!("{:#?}", result);
    assert_eq!(result.len(), 300);
  }

  #[test]
  fn test_integration() {
    let source_code = "
// this is a comment1
/// this is a doc comment1
/* this is a block comment1
second line! */
impl MyStruct {
    fn new() -> Self {
        MyStruct {}
    }
}
let x = if true {
    5
} else {
    10
};
// 222222222222222222
/// 22222222222oc comment1
/* this2is a block comment2
second 2ine! */
impl My2Struct {
    fn n2ew() -> Self {
        My2Struct {}
    }
}
";

    let comments = extract_comments(source_code);
    let constructs = extract_code_constructs(source_code);
    let declarations = extract_declarations(source_code);
    // println!("{:#?}", comments);
    println!("{:#?}", constructs);
    // println!("{:#?}", declarations);
    assert!(false)
  }

  #[test]
  fn test_integration2() {
    let source_code = r####"
#[cfg_attr(not(test), rustc_diagnostic_item = "HashMap")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_insignificant_dtor]
pub struct HashMap<K, V, S = RandomState> {
    base: base::HashMap<K, V, S>,
}

impl<K, V> HashMap<K, V, RandomState> {
    /// Creates an empty `HashMap`.
    ///
    /// The hash map is initially created with a capacity of 0, so it will not allocate until it
    /// is first inserted into.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::new();
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn new() -> HashMap<K, V, RandomState> {
        Default::default()
    }

    /// Creates an empty `HashMap` with at least the specified capacity.
    ///
    /// The hash map will be able to hold at least `capacity` elements without
    /// reallocating. This method is allowed to allocate for more elements than
    /// `capacity`. If `capacity` is 0, the hash map will not allocate.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::with_capacity(10);
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn with_capacity(capacity: usize) -> HashMap<K, V, RandomState> {
        HashMap::with_capacity_and_hasher(capacity, Default::default())
    }
}"####;
    // let comments = extract_comments(source_code);
    // let constructs = extract_code_constructs(source_code);
    // let declarations = extract_declarations(source_code);
    // println!("{:#?}", comments);
    // println!("{:#?}", constructs);
    // println!("{:#?}", declarations);

    extract_parsed_nodes(source_code);
    assert!(false)
  }
}
