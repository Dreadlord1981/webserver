use clap::Parser;
use webserver::{api::init, cli::Args, layers::RewriteService};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args = Args::parse();

    let (app, listener) = init(&args).await?;

    let svc = RewriteService {
        inner: app.into_service(),
    };

    axum::serve(listener, tower::make::Shared::new(svc))
        .with_graceful_shutdown(webserver::api::shutdown_signal())
        .await?;

    Ok(())
}
