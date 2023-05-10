use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, about, version)]
pub struct Args {
    #[arg(short, long, help = "Enable verbose logging", default_value_t = false)]
    pub verbose: bool,
    #[arg(short, long, help = "Magento 2 working directory")]
    pub working_directory: Option<std::path::PathBuf>,
}

pub fn parse_args() -> Args {
    Args::parse()
}
