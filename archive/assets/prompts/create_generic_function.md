Act as an expert prompt engineer

# The Goal:

# Context:
We are developing a CLI application in Rust that accesses GPT
the application has various function calls that provide GPT access with access to command line programs.

These files contain function calls which call a command line app, but which are not generic
- pcre2grep_function.rs
- patch_files_function.rs

We are trying to create a new file named generic_cli_function.rs

# Steps
- Read the 3 provided files
  1. Review their functionality
  2. Determine what would be necessary in order to generalize their functionality
- Review work that has already been completed.
  1. Some work may have already been completed. Review the progress and proceed from there.
- Create a plan to achieve the goal
  1. Designate file paths for any files that will be generated by this plan
  2. Create a list of every function signature that will end up in the source
  3. Review the plan to determine if it will sufficiently accomplish the goal
- Create a new prompt file that contains this plan
  1. Create an "Act as a..." line that will ensure that GPT will be able to achieve the goal in source on disk
  2. Include instructions that will ensure that GPT will follow the code requirements below

# Code Requirements
- use ./.session_data if you need to save any interim files
- The new function must integrate with the existing application in the same way the existing functions do.
- They should allow the user to define a subset of arguments that the GPT app can pass to the function call
- Ensure you write all code in idiomatic rust.
- Ensure you write tests for your code.
- Ensure the code will compile
- Ensure the code is well documented
- Ensure you provide complete comprehensive code
- DO NOT PROVIDE ANY PLACEHOLDERS.
- Complete all code and complete the request in its entirety in as few steps as possible
- Write any intermediate files in .session_data
- Only write the completed source into ./src/app/functions and ./tests
- DO NOT UPDATE ME ON YOUR PROGRESS.
- Create files with the create_file command
- overwrite existing files by using overwrite:true
- respond with either file changes, or a request for more information, or a status message that is at most 5 words long.
