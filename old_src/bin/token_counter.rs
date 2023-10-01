use clap::Parser;
use std::fs;
use std::io::{self, Read};
use tiktoken_rs::tokenizer::get_tokenizer;
use tiktoken_rs::get_bpe_from_tokenizer;

#[derive(Parser)]
#[clap(
    version = "1.0",
    author = "Your Name",
    about = "Program that Counts the number of tokens in the provided text"
)]
struct Opts {
    #[clap(short = 'f', long = "file", help = "File which contains the text")]
    file: Option<String>,
    #[clap(short = 'm', long = "model", default_value = "gpt-4", help = "The openai model for which to tokenize the text for")]
    model: String,
}

fn main() {
    let opts: Opts = Opts::parse();

    let tokenizer = get_tokenizer(&opts.model).unwrap();

    let mut content = String::new();

    match opts.file {
        Some(file) => {
            match fs::read_to_string(&file) {
                Ok(data) => content = data,
                Err(e) => {
                    eprintln!("Failed to read file: {}", e);
                    std::process::exit(1);
                }
            }
        },
        None => {
            match io::stdin().read_to_string(&mut content) {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("Failed to read from stdin: {}", e);
                    std::process::exit(1);
                }
            }
        }
    };
    let bpe = get_bpe_from_tokenizer(tokenizer).unwrap();
    let tokens = bpe.encode_with_special_tokens(&content);
    println!("Number of tokens: {}", tokens.len());
}