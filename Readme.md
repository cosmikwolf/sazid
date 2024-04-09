![Project Banner or Logo](docs/sazid_banner_image.png)

# Sazid

[![CI](https://github.com/cosmikwolf/sazid/workflows/CI/badge.svg)](https://github.com/cosmikwolf/sazid/actions)

**Currently a work in progress**
--- embeddings are not hooked up at the moment ---

Sazid is an interactive LLM interface written in Rust that provides:

- Full Color Terminal User Interface
- Embeddings database implemented using postgres, pgvector and diesel-async
- Async Enabled UI, API, and Database
- Function Call tools that enable LLM to read and modify code

## **Table of Contents**

- [**Features**](#features)
- [**Getting Started**](#getting-started)
  - [**Prerequisites**](#prerequisites)
  - [**Installation**](#installation)
- [**Usage**](#usage)
- [**Contributing**](#contributing)
- [**License**](#license)
- [**Acknowledgements**](#acknowledgements)

## **Feature Roadmap**

- [x] **Interactive CLI**: Engage with a user-friendly command-line interface.
- [x] **GPT Integration**: Seamlessly connect with OpenAI's GPT model for interactive chats.
- [x] **Language Server Integration**: Allow GPT to traverse and modify your codebase syntax tree
- [x] **Vector Database**: Connection to a vector enabled postgres database for local data access
- [x] **Text File Embeddings**: Generate embeddings from text files and load them into the vector database
- [x] **Chat Session Management**: Start new sessions or continue previous ones with ease.
- [ ] **Non OpenAI LLM Support** Connect with any LLM hosted with the OpenAI API
- [ ] **File Ingestion**: Import and process a variety of file formats
- [ ] **[Retrieval Augmented Generation](https://arxiv.org/pdf/2005.11401.pdf)**: Improve the quality of your LLM responses by sending customizable and relevant context
- [ ] **Prompt Manager**: Manage a set of reliable prompts that have specific tasks and rate their effectiveness
- [ ] **Recursive Prompt Functions** Allow GPT to achieve complex goals by breaking it down into smaller tasks using managed prompts
- [ ] **nvim + vscode plugin** Interact with Sazid directly in neovim or vscode
- [ ] **Interaction with build tools** enable GPT to retrieve and fix code warnings
- [ ] **Integration with debugging tools** Let GPT help you debug runtime issues
- [ ] **Integration with embedded hardware diagnostics** Give GPT an awareness of the behavior of embedded code by interfacing directly with a logic analyzer

## **Getting Started**

### **Prerequisites**

Before you start, ensure you have the following installed:

- [Rust](https://www.rust-lang.org/)
- [Cargo](https://doc.rust-lang.org/cargo/)
- Docker
- Diesel

### **Installation**

- configure your OPENAI_API_KEY env variable

### Vector DB Setup

#### Start the database service

- docker-compose up -d

#### Stop the database service

- docker-compose down

#### Run database migrations

- diesel migration run

## Usage

Sazid is currently a work in progress.

It can currently be used as an LLM interface with function calls that allow GPT to:

- Read files
- Write files
- pcre2grep files

There is also disabled code that enables GPT to use sed and a custom function to modify files directly, but I have found that GPT consistently makes annoying mistakes that are extremely frustrating to resolve when it is forced to use regular expressions and line numbers to modify files.

This is the motivating factor behind implementing tree-sitter to give GPT the capability of full syntax awareness and direct syntax tree modification so it can be specific about its modifications.

There are still some basic features I need to complete in order for a non-frustrating user experience

- Session Management
- Copy / Paste from the TUI

## Retrieval Augmented Generation and the Vector Database

When reading and writing source code using GPT, the context can quickly balloon, making long running API sessions become between linearly and exponentially more expensive to submit.
A Retrieval Augmented Generation system converts your prompt into an embedding vector, then queries a vector database for text that has similar meaning, and then includes a specified amount of text as the context for the prompt.

By loading documentation for all technologies involved into a vector database, with proper tagging to allow the user to enable and disable certain domains, we can significantly reduce the number of tokens per call, while still providing GPT the proper context with each prompt.

Each chat interaction will also be included in this database, so previous conversations can be automatically included in the same way.

This allows us to take advantage of the large input context capabilities of models like GPT4-Turbo, while also ensuring that we are not wasting tokens, and increasing risk of hallucinations with irrelevant context.

## Goals

Some of the goals I hope for this project to achieve:

- Automated datasheet to driver pipeline
- Automated unit test generation to ensure full coverage, including edge cases
- A debug assistant that can provide insights into
- A hardware test generator with generation of interface code for a test harness
- C4 systems diagram generation based on code analysis
- Single prompt port of a codebase to a new language

## Embedded Focused

The goal of Sazid is to provide support to develop for embedded hardware platforms.
Frequently embedded engineers spend many hours debugging hardware issues by analyzing trace logs, reading debug registries and stimulating GPIO inputs in order to zero in on what may be a single misplaced character in a driver for a specific IC.

What if an LLM could have

- complete awareness of datasheets for all ICs in the PCB
- complete semantic awareness of the source code, including drivers
