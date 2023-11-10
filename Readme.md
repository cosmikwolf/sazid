
![Project Banner or Logo](docs/sazid_banner_image.png)
# **Sazid: Ascended Rust Development**

[![CI](https://github.com/cosmikwolf/sazid/workflows/CI/badge.svg)](https://github.com/cosmikwolf/sazid/actions)

Sazid is an interactive chat application that connects users with OpenAI's GPT model. With the power of GPT and a user-friendly command-line interface, it allows seamless conversations and can process and import various file types.

## **Table of Contents**

- [**Sazid: Ascended Rust Development**](#sazid-ascended-rust-development)
  - [**Table of Contents**](#table-of-contents)
  - [**Features**](#features)
  - [**Getting Started**](#getting-started)
    - [**Prerequisites**](#prerequisites)
    - [**Installation**](#installation)
  - [**Usage**](#usage)
  - [**Contributing**](#contributing)
  - [**License**](#license)
  - [**Acknowledgements**](#acknowledgements)

## **Features**

- [x] **Interactive CLI**: Engage with a user-friendly command-line interface.
- [x] **GPT Integration**: Seamlessly connect with OpenAI's GPT model for interactive chats.
- [ ] **Chat Session Management**: Start new sessions or continue previous ones with ease.
- [ ] **File Ingestion**: Import and process a variety of file formats
- [ ] **Vector Database**: Connection to a vector enabled postgres database for local data access

## **Getting Started**

### **Prerequisites**

Before you start, ensure you have the following installed:

- [Rust](https://www.rust-lang.org/)
- [Cargo](https://doc.rust-lang.org/cargo/)
- cargo-pgrx

### **Installation**
- configure your OPENAI_API_KEY env variable


### Vector DB Setup

adapted from: https://github.com/tensorchord/pgvecto.rs/blob/0efe49d97623656c2e26452df473c6518786dc06/docs/install.md

- cargo install cargo-pgrx
- cargo pgrx init
- brew install postgresql@15
- initdb /usr/local/var/postgres

