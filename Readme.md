
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

- **GPT Integration**: Seamlessly connect with OpenAI's GPT model for interactive chats.
- **Chat Session Management**: Start new sessions or continue previous ones with ease.
- **File Ingestion**: Import and process a variety of file formats.
- **Interactive CLI**: Engage with a user-friendly command-line interface.
- **Error Handling**: Robust mechanisms to handle potential issues, ensuring smooth user experiences.

## **Getting Started**

### **Prerequisites**

Before you start, ensure you have the following installed:

- [Rust](https://www.rust-lang.org/)
- [Cargo](https://doc.rust-lang.org/cargo/)

You'll also need API keys for OpenAI's GPT. Store them securely as environment variables.

### **Installation**

1. Clone the repository:

```bash
git clone https://github.com/your_username/sazid.git
cd sazid
```

2. Build the project:

```bash
cargo build --release
```

3. Run the application:

```bash
cargo run --release
```

## **Usage**

To start a new chat session:

```bash
sazid -n
```

To continue a previous session:

```bash
sazid -c path_to_session_file
```

To import a file or directory:

```bash
sazid -i path_to_file_or_directory
```

For a complete list of options:

```bash
sazid --help
```

## **Contributing**

We welcome contributions! Please read [CONTRIBUTING.md](path_to_contributing.md) for details on our code of conduct and submission process.

## **License**

This project is licensed under the MIT License - see the [LICENSE.md](path_to_license.md) file for details.

## **Acknowledgements**

- [OpenAI](https://www.openai.com/) for providing the GPT model.
- [Rust community](https://www.rust-lang.org/) for their comprehensive documentation and support.