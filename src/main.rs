use webserver::{api::init, cli::Args, layers::RewriteService};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {

	let args = Args::parse();

	let (app, listener) = init(&args).await?;

	let svc = RewriteService{
		inner: app.into_service()
	};

	axum::serve(listener, tower::make::Shared::new(svc)).await?;

	Ok(())
}