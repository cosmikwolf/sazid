### **Sazid: An Interactive GPT Chat Application**

#### **Functional Requirements**:

1. **GPT Integration**:
    - Utilize the `async_openai` library to integrate with OpenAI's GPT model.
    - Establish a connection to the GPT API using provided API keys from the environment.
    - Send user messages to the GPT model and retrieve generated responses.
    - Handle different message roles, specifically User and Assistant.

2. **Chat Session Management**:
    - Provide command-line options to:
        - Start a new chat session (`-n` or `--new` flag).
        - Continue a previously stored session (`-c` or `--continue` flag with the session file as an argument).
    - Save chat sessions within the `session_data` directory, ensuring the data is structured into sub-directories like `ingested` and `processed`.
    - Automatically pick the latest session if no specific session is mentioned by the user.
    - Store the last-used session's filename in a text file named `last_session.txt`.
    - Delete a specific session if needed.

3. **File Ingestion and Data Logging**:
    - Implement an ingestion mechanism to handle files provided by the user.
    - Ensure the ingested files are stored in the `session_data/ingested` directory.
    - Log details of the ingested data in a structured format, including chunk numbers and file paths.
    - Store the ingested files with a naming convention that includes the session ID for easy identification.

4. **User Interaction**:
    - Implement a command-line interface for users to interact with the application.
    - Accept user input for messages to be sent to GPT.
    - Allow session exit using the command "exit", "quit", or `Ctrl+C`.
    - Implement an import feature to process specified files or directories (using the `-i` or `--import` flag followed by the file or directory path).

5. **Message Display**:
    - Display messages in the command-line interface with clear distinction between user and GPT messages.
    - Color-code GPT messages in green for easy differentiation.
    - Display a startup message when the application begins.
    - Provide exit messages when the application terminates.
    - Display import-related messages to inform the user of the import process status, including success, failure, or skipped statuses.

6. **File Chunking and Processing**:
    - Process various file types, including PDFs and text files.
    - Extract content from PDFs, keeping track of pages and any extraction errors.
    - For text files, chunk the content line by line.
    - For binary files, detect their type and provide appropriate messages.
    - For PDF files, detect their type and extract text content as needed.
    - Inform the user if a provided file appears to be binary and cannot be processed.

7. **Command-line Interface**:
    - Implement a robust command-line argument system using the `clap` library.
    - Provide flags to allow users to:
        - Start a new chat session (`-n` or `--new`).
        - Continue a specific session (`-c` or `--continue`).
        - Import a file or directory for processing (`-i` or `--import`).
    - Display version, author, and other metadata information when queried.

8. **Data Serialization and Storage**:
    - Serialize chat messages in JSON format for storage in session files.
    - Ensure that each message stored consists of:
        - Role (User or Assistant).
        - Content of the message.
    - Deserialize JSON data when loading from session files.

9. **Timestamp and Random Hash Management**:
    - Generate timestamps based on the system's local time.
    - Use the `rand` library to create random hashes for session filenames.
    - Utilize timestamps and random hashes for naming session files.

10. **Modularity and Code Structure**:
    - Modularize functionalities into distinct modules:
        - GPT Integration.
        - Session Management.
        - User Interface.
        - File Chunking.
        - PDF Text Extraction.
    - Ensure that each module has dedicated functionality and minimizes dependencies on other modules.

#### **Non-functional Requirements**:

1. **Scalability**:
    - Design the application to efficiently handle multiple users without degrading performance.

2. **Responsiveness**:
    - Ensure prompt feedback to user input and provide timely GPT responses.

3. **Maintainability**:
    - Maintain a well-organized and documented codebase, enabling straightforward future modifications.

4. **Security**:
    - Do not hardcode API keys; instead, securely manage them using environment variables.
    - Implement safety measures to prevent unauthorized access to stored chat sessions.

5. **Reliability**:
    - Incorporate mechanisms to recover from potential crashes, ensuring continued operations with minimal data loss.

6. **Portability**:
    - Design the application to be platform-independent, ensuring it runs on various operating systems without major alterations.

7. **Usability**:
    - Make the command-line interface intuitive and user-friendly, ensuring users can easily understand and follow instructions.

8. **Dependency Management**:
    - Rely on third-party libraries such as `rustyline`, `clap`, `async_openai`, `lopdf`, `serde_json`, `serde`, `chrono`, `rand`, and `owo_colors`. Ensure these libraries are maintained and updated as needed.

9. **Error Handling**:
    - Implement comprehensive error-handling mechanisms to address potential issues related to:
        - GPT API connections.
        - File read/write operations.
        - Session management.
        - File import processes.
    - Display user-friendly error messages, informing users about the nature of any encountered error.


requirements update 8/29

1. **FileChunker Refactoring**:
   - The module no longer determines file types; this is left to the calling function.
   - Introduced a new error variant `FileChunkerError::ChunkingError(String)` to provide a description of specific chunking errors.
   - Error handling has been improved with custom error messages for different types of issues.

2. **FileChunker Methods**:
   - `chunkify_file`: This method chunks the content of a file based on its type (PDF, text, etc.).
   - `extract_file_text`: Extracts the text content of a file. This method handles both plain text and PDF files.
   - `is_pdf_file` and `is_binary_file`: Helper functions to determine the file type.
   - `chunk_content_by_tokens`: Splits content into chunks based on token count. Each token is a word, and this function ensures that chunks do not split words. This is the main chunking logic for the OpenAI API's token-based chunking requirement.

3. **Chunking Logic**:
   - The content is split into tokens (words).
   - Chunks are created by iterating through the tokens and counting their characters until the desired token count is reached for a chunk. 
   - The method returns these chunks as a `Vec<String>`.

4. **Tests**:
   - Updated tests to reflect the refactored logic.
   - Added print statements to the tests to display chunks during test runs.
   - The tests now check for proper chunking and error handling.

5. **Miscellaneous**:
   - We discussed the name of the `chunk_file_content` method, which was suggested to be renamed to `retrieve_file_text` or something similar since it's not directly chunking.
   - We introduced a new function in `FileChunker` that would take a file and produce the chunks so that `handle_ingest` doesn't have to call the chunker multiple times.
   - We switched from using a generated PDF to using the PDFs in `tests/data`.
   - We clarified that the token-based chunking is in line with the OpenAI API's requirement.

updated requirements 8/30

1. **Session Management and File Paths** (`session_manager.rs`)
    - The system should use idiomatic Rust file and directory handling using the `Path` and `PathBuf` types from the `std::path` module.
    - Session filenames should be derived from the `session_id` attribute present in the `SessionManager` struct, and each should have a `.json` extension.
    - The system should store session files in a directory specified as `./data/sessions`.
    - The system should maintain logs of ingested files in a directory specified as `./data/ingested`.

2. **Token Counting and API Limit Management**
    - Integrate the `tiktoken-rs` crate in conjunction with the `async_openai` feature.
    - Before sending messages to the OpenAI API, the system should ensure the token count of messages does not surpass the model's token limitations.

3. **Model Handling and API Communication** (`gpt_connector.rs`)
    - The system should define constants that represent models available from the OpenAI API.
    - All API calls made by `GPTConnector` should utilize the `client` variable from its struct to prevent initializing a new client with each call.
    - Implement exponential backoff using `async_openai` to enhance error handling and provide resilience against rate limits.

4. **Configuration Management Using `config` Crate**
    - The application should utilize the `config` crate to manage its configuration.
    - Configuration settings should be stored in a file named `Settings.toml`.
    - The system should support a default model and a fallback model, both specified in the configuration.
    - If the default model is inaccessible, the system should attempt to access the fallback model.
    - The application should terminate if neither the default nor the fallback models are accessible.

5. **Refactoring of Model Definitions**
    - Remove the old `Models` struct and associated constants from `session_manager.rs`.
    - Introduce a `Model` struct to represent individual models and their attributes.
    - Define module-level constants, such as `GPT3_TURBO` and `GPT4_TURBO`, to represent predefined models.

6. **Idiomatic Rust Code Structure**
    - Both `main.rs` and `session_manager.rs` should be structured in an idiomatic Rust order:
        - `use` statements (imports) should be placed at the top.
        - Constants should be defined after imports.
        - Structs, enums, and other type definitions should follow.
        - `impl` blocks and function definitions should conclude the file.

updated requirements #2 8/30

Absolutely! Here's a detailed list of the updated functional requirements based on the tasks and discussions from this session:

### **Configuration Management Using the `config` Crate**

1. **Dependencies**:
    - **Requirement**: The application must include specific crates to handle configuration management.
    - **Detail**:
        - The application's `Cargo.toml` file should include the `config` crate for configuration management.
        - The `serde` and `serde` crates should be added for serialization and deserialization of configuration data.

2. **Configuration File**:
    - **Requirement**: The application must have a centralized configuration file named `Settings.toml`.
    - **Detail**:
        - The `Settings.toml` file should be located in the root directory.
        - Within this file, there should be two model configurations: `default` and `fallback`.
        - Each configuration specifies the name of a model, e.g., "gpt-4" or "gpt-3.5-turbo".

3. **Configuration Loading in `main.rs`**:
    - **Requirement**: The main application must be able to load and utilize configurations from `Settings.toml`.
    - **Detail**:
        - There should be imports from the `config` and `serde` crates.
        - Two structs, `ModelsConfig` and `ModelConfig`, should be defined to match the structure of the `Settings.toml` file.
        - A function named `load_config` should be implemented to load the configuration. It should:
            - Initialize a default config.
            - Merge data from `Settings.toml`.
            - Deserialize the merged data into the `ModelsConfig` struct.
        - The `main` function of the application should:
            - Load the configuration using `load_config()`.
            - Check access to the model specified in the `default` configuration.
            - If the default model is inaccessible, it should attempt to access the model in the `fallback` configuration.
            - The application should exit if neither model is accessible.

### **Refactoring Model Definitions**

1. **Model Struct & Constants**:
    - **Requirement**: The application should have a new `Model` struct definition and associated constants.
    - **Detail**:
        - Both `session_manager.rs` and `main.rs` should define the `Model` struct with fields `name`, `endpoint`, and `token_limit`.
        - Module-level constants `GPT3_TURBO` and `GPT4_TURBO` should be defined with specific details about the models.
        - The old `Models` struct and associated constants should be removed from `session_manager.rs`.

### **General Requirement**

1. **Clean and Idiomatic Rust Code**:
    - **Requirement**: The application code, should be organized following idiomatic Rust practices.
    - **Detail**:
        - `use` statements (imports) should be at the top.
        - Module-level constants should follow.
        - Structs, enums, and other type definitions should come next.
        - Finally, `impl` blocks and function definitions should be at the end.