use webserver::{api::init, cli::Args, config::WebConfig, db::User, layers::RewriteLayer};
use clap::Parser;
use tokio::{fs::OpenOptions, io::AsyncReadExt};
use anyhow::anyhow;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {

	let args = Args::parse();

	let file_result = OpenOptions::new().read(true).open("examples/proxy/webconfig.toml").await;

	if let Ok(mut file) = file_result {

		let mut buffer = String::from("");

		file.read_to_string(&mut buffer).await?;

		let config: WebConfig = toml::from_str(&buffer)?;
		let server = &config.server;
		
		let app = init(&config, &args).await?;

		let listener = tokio::net::TcpListener::bind(format!("localhost:{}", server.port)).await?;

		println!("Serving at http://localhost:{}", server.port);

		let user = User {
			username: "PNO".into()
		};

		let app = app.layer(RewriteLayer {state: user.clone()});

		axum::serve(listener, tower::make::Shared::new(app)).await?;
	}
	else {
		return Err(anyhow!("webconfig.toml not found mut be place in root of server"));
	}

	Ok(())
}