
use std::{path::PathBuf, sync::Arc};

use webserver::{api::init, cli::Args, config::{RewriteWithState, WebConfig}, db::User};
use clap::Parser;
use expand_env_vars::expand_env_vars;
use tokio::{fs::OpenOptions, io::AsyncReadExt};
use anyhow::anyhow;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {

	let args = Args::parse();

	let root = if let Some(path) = &args.root {
		if path.starts_with("%") {
			expand_env_vars(path)?
		}
		else {
			shellexpand::env(&path)?.into()
		}
	}
	else {
		".".into()
	};

	let root_path = PathBuf::from(&root);

	let file_result = if root_path.exists() {

		let file_path = root_path.join("webconfig.toml");

		if file_path.exists() {
			let _ = std::env::set_current_dir(&root);
		}

		OpenOptions::new().read(true).open(file_path).await
	}
	else {
		OpenOptions::new().read(true).open("webconfig.toml").await
	};

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

		let svc = RewriteWithState { inner: app.into_service(), state: Arc::new(user) };

		axum::serve(listener, tower::make::Shared::new(svc)).await?;
	}
	else {
		return Err(anyhow!("webconfig.toml not found mut be place in root of server"));
	}

	Ok(())
}