use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Args {
    /// Expose the browser
    #[arg(short, long, default_value = "false")]
    pub gui: bool,

    /// AWS SSO profile
    #[arg(short, long)]
    pub profile: Option<String>,
}

impl Args {
    pub fn new() -> Self {
        Self::parse()
    }
}
