use webserver::{api::init, cli::Args};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {

	let args = Args::parse();

	let (app, listener) = init(&args).await?;

	axum::serve(listener, app.into_make_service()).await?;

	Ok(())
}