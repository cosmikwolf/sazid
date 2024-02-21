# Act as a Rust CLI Application Developer: Create a sed function call for my GPT app

## Goal:
Develop a function call for the current application that provides GPT the ability to use sed that meets the Function Call Requirements defined below.

## File names:

- main source file
  - ./src/app/functions/sed_command.rs
- tests file:
- ./tests/sed_command_tests.rs

## Context:
The application interacts with GPT and command line programs via function calls.
Existing function calls that should be referenced are in: `pcre2grep_function.rs` and `patch_files_function.rs`
These files

## Iteraton Steps:
1. Analyze the work that has already been completed in the main source file and test file
2. consult existing files that contain my application's function call abilities: `pcre2grep_function.rs` and `patch_files_function.rs`
3. Determine the next step towards completing the coal
4. Use the `create_file` command with `overwrite:true` to overwrite the existing code with updated code that gets us closer towards completing the goal

## Function Call Requirements:

Based on the analysis of `pcre2grep_function.rs` and `patch_files_function.rs`, the following requirements are set for additional function calls within the application:

1. **Interface Consistency**: Implement a consistent interface with `ModelFunction` trait, including `init`, `call`, and `command_definition` methods.

2. **Parameter Handling**: Handle both required and optional parameters, validate inputs, and use existent validation utilities.

3. **Error Handling**: Utilize `ModelFunctionError` for consistent error mapping and handling.

4. **Command Execution**: Wrap system commands using `std::process::Command` and handle command results, including stdout and stderr.

5. **Return Types**: Return a result with an optional output string for clear communication of success or errors.

6. **Serialization**: Ensure data structures for function metadata (e.g., `name`, `description`) derive `Serialize` and `Deserialize`.

7. **Documentation**: Provide clear documentation for the function, following Rust's standards with descriptions and examples.

8. **Compile and Test**: Make sure the function compiles and is accompanied by tests to validate its functionality.

9. **Session Configuration**: Consider session configuration in the function's implementation.

10. **Patch Handling**: For modification functions like patching, adhere to existing formats and instruction clarity for creating input files.

11. **CLI Integration**: Allow for CLI-specific arguments in functions invoking CLI tools, correctly handle paths, and integrate into the session workflow.

These requirements aim to maintain consistency and compatibility with the existing application architecture and coding practices.
