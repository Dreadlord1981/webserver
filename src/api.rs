
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::anyhow;
use axum::http::{HeaderName, HeaderValue};
use axum::response::Response;
use axum::extract::{Request, State};
use axum::middleware::{Next, from_fn, from_fn_with_state};
use axum::Router;
use axum::routing::{get, post};
use axum_proxy::{AppendPrefix, TrimPrefix};
use expand_env_vars::expand_env_vars;
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tower_http::{compression::CompressionLayer, services::ServeDir};
use tracing::info;

use crate::cli::Args;
use crate::layers::{RewriteLayer, TrimToWildcard};
use crate::{config::WebConfig, plugins::{plugins_get, plugins_post}};

pub async fn init(args: &Args) -> Result<(Router, TcpListener), anyhow::Error> {

	let root = if let Some(path) = &args.root {
		if path.starts_with("%") {
			expand_env_vars(path)?
		}
		else if path.starts_with("~") {
			shellexpand::full(&path)?.into()
		}
		else {
			shellexpand::env(&path)?.into()
		}
	}
	else {
		".".into()
	};

	let root_path = PathBuf::from(&root);

	let resolved = root_path.canonicalize().unwrap();

	let mut root_path = PathBuf::from(&resolved);

	let mut file = if root_path.exists() {

		let file_path = if root_path.is_file() {

			let path = root_path.clone();

			root_path = root_path.parent().unwrap().to_path_buf();

			path
		}
		else {
			root_path.join("webconfig.toml")
		};

		if file_path.exists() {
			let _ = std::env::set_current_dir(&root);
		}
		else {
			return Err(anyhow!("Toml not found mut be place in root of server"));
		}

		OpenOptions::new().read(true).open(file_path).await.unwrap()
	}
	else {
		return Err(anyhow!("Invalid path"));
	};

	let mut buffer = String::from("");

	file.read_to_string(&mut buffer).await?;

	let config: WebConfig = toml::from_str(&buffer)?;

	let config = config.clone();

	tracing_subscriber::fmt::init();
	
	let mut app = Router::new();

	app = app.layer(CompressionLayer::new().gzip(true));

	let server = config.server.clone();

	let mut root_used = false;
	
	if let Some(routes) = server.route {

		for route in routes.iter() {

			if route.ifs.is_some() {

				let ifs = route.ifs.as_ref().unwrap();
				
				let path: String = if ifs.starts_with("%") {
					expand_env_vars(ifs)?
				}
				else {
					shellexpand::env(ifs)?.into()
				};

				let dir = ServeDir::new(path);
				app = app.nest_service(&route.path, dir);
			}
			else {

				let route_address = if let Some(route_address) = &route.address {
					route_address.to_string()
				}
				else {
					server.address.clone()
				};

				if route.path == "/" {
					root_used = true;
				}

				let https = if let Some(val) = route.https {
					val
				}
				else {
					let mut result = false;

					if let Some(server_val) = server.https {
						result = server_val
					}

					result
				};

				app = if https {
					let proxy = if let Some(val) = route.strip && val {
						
						axum_proxy::builder_https(route_address)?.build(TrimToWildcard(route.path.clone()))
					}
					else {
						axum_proxy::builder_https(route_address)?.build(TrimToWildcard("".into()))
					};
					app.route_service(&route.path, proxy)
				}
				else {
					let proxy = axum_proxy::builder_http(route_address)?.build(AppendPrefix(""));
					app.route_service(&route.path, proxy)
				};
			}
		}
	}

	if !server.address.is_empty() {
		app = if let Some(val) = server.https && val {
			let proxy = axum_proxy::builder_https(server.address.clone())?.build(TrimPrefix("out"));
			app.route_service("/out/{*path}", proxy)
		}
		else {
			let proxy = axum_proxy::builder_http(server.address.clone())?.build(TrimPrefix(""));
			app.route_service("/out/{*path}", proxy)
		};
	}

	app = app.route("/plugins/{*path}", get(plugins_get));

	app = app.route("/plugins/{*path}", post(plugins_post));

	if !root_used {
		let root_folder = ServeDir::new(root_path)
			.precompressed_gzip()
			.precompressed_br();

		app = app.fallback_service(root_folder);
	}

	app = app.layer(from_fn(log_request));

	app = app.route_layer(from_fn_with_state(config.clone(), set_headers));

	app = app.layer(RewriteLayer);

	let listener = tokio::net::TcpListener::bind(format!("localhost:{}", server.port)).await?;

	println!("Serving at http://localhost:{}", server.port);

	Ok((app, listener))

}

async fn log_request(req: Request, next: axum::middleware::Next) -> axum::response::Response {
    // Log the request details
    info!("{} {}", req.method(), req.uri());

    // Continue to the next middleware or handler
    next.run(req).await
}

async fn set_headers(
	State(state): State<WebConfig>,
	request: Request,
	next: Next
) -> Response {

	let mut response = next.run(request).await;
	let server = state.server;

	let headers = response.headers_mut();

	if let Some(route_headers) = server.headers {

		for h in route_headers {
			headers.insert(HeaderName::from_str(&h.key).unwrap(), HeaderValue::from_str(&h.value).unwrap());
		}
	}

	response
}