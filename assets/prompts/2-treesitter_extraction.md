# Iterative Approach for Tree-sitter Code Extraction with Iterative Saves

Develop Rust code to parse Rust source code with Tree-sitter and iteratively extract elements for a vector database. Each iteration focuses on a specific set of constructs and saves the incremental code into separate files under `./src/app/embeddings/` with indicative titles.

## Iterations

### Iteration 1: Basic Setup and Comment Extraction
- File: `iteration1_basic_setup_and_comments.rs`
- Setup Tree-sitter with the Rust grammar.
- Parse a single Rust file to extract documentation comments.
- Serialize comments into a structured format (e.g., JSON).
- Save progress in `./src/app/embeddings/iteration1_basic_setup_and_comments.rs`.

### Iteration 2: Declaration Extraction
- File: `iteration2_declarations.rs`
- Build upon Iteration 1 to include function, struct, enum, and trait declarations.
- Serialize declarations and update serialization to include them.
- Save progress in `./src/app/embeddings/iteration2_declarations.rs`.

### Iteration 3: In-depth Code Constructs
- File: `iteration3_code_constructs.rs`
- Parse more complex elements like implementations, definitions, control structures.
- Include literals, non-doc comments, attributes, and macros in extraction.
- Serialize new constructs, enhancing structured format.
- Save progress in `./src/app/embeddings/iteration3_code_constructs.rs`.

### Iteration 4: Data Representation for ML
- File: `iteration4_data_representation.rs`
- Convert code constructs into a suitable vector representation for machine learning.
- Decide context, scope, and relationships for embeddings.
- Save progress in `./src/app/embeddings/iteration4_data_representation.rs`.

### Iteration 5: Database Schema and Storage
- File: `iteration5_database_schema_and_storage.rs`
- Design a database schema for vector data storage.
- Implement and test database storage for serialized constructs.
- Save progress in `./src/app/embeddings/iteration5_database_schema_and_storage.rs`.

### Iteration 6: Refinement and Testing
- File: `iteration6_testing_and_refinement.rs`
- Conduct a thorough test with various Rust projects.
- Refine processes based on feedback.
- Optimize database interaction and save progress.
- Save final code in `./src/app/embeddings/iteration6_testing_and_refinement.rs`.

## Final Compilation
- Compile all iterations into a cohesive application.
- Document each file with clear explanations for maintainability.
- Name each incremental save file with the respective iteration title provided.