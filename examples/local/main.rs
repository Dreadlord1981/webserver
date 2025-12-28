

use webserver::{api::init, cli::Args};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {

	let mut args = Args::parse();
	args.root = Some("./examples/local/www".into());

	let (app, listener) = init(&args).await?;

	axum::serve(listener, app.into_make_service()).await?;

	Ok(())
}