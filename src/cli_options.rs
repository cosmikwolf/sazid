
use clap::Parser;
use std::ffi::OsString;

#[derive(Parser)]
#[clap(
    version = "1.0",
    author = "Your Name",
    about = "Interactive chat with GPT"
)]
pub struct Opts {
    #[clap(
        short = 'm',
        long,
        value_name = "MODEL_NAME",
        help = "Specify the model to use (e.g., gpt-4, gpt-3.5-turbo-16k)"
    )]
    pub model: Option<String>,

    #[clap(
        short = 'l',
        long = "list-models",
        help = "List the models the user has access to"
    )]
    pub list_models: bool,

    #[clap(short = 'n', long, help = "Start a new chat session")]
    pub new: bool,

    #[clap(short = 'c', long, help = "Continue from a specified session file")]
    pub continue_session: Option<String>,

    #[clap(
        short = 'i',
        long,
        value_name = "PATH",
        help = "Import a file or directory for GPT to process"
    )]
    pub ingest: Option<OsString>,
}
