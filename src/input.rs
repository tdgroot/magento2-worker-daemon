use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, about, version)]
pub struct Args {
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,
    #[arg(short, long)]
    pub working_directory: Option<std::path::PathBuf>,
}

pub fn parse_args() -> Args {
    Args::parse()
}
