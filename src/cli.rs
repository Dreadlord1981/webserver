use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
	#[arg(long="root", short='r', help="Root folder of server")]
	pub root: Option<String>,

	#[arg(long="verbose", short='v', help="Verbose mode")]
	pub verbose: bool
}