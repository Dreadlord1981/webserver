
use std::str::FromStr;

use axum::http::{HeaderName, HeaderValue, header};
use axum::response::Response;
use axum::extract::{Request, State};
use axum::middleware::{Next, from_fn, from_fn_with_state};
use axum::Router;
use axum::routing::{get, post};
use axum_proxy::AppendPrefix;
use expand_env_vars::expand_env_vars;
use tower_http::services::ServeFile;
use tower_http::{compression::CompressionLayer, services::ServeDir};
use tracing::info;

use crate::cli::Args;
use crate::layers::TrimToWildcard;
use crate::{config::WebConfig, plugins::{plugins_get, plugins_post}};

pub async fn init(config: &WebConfig, args: &Args) -> Result<Router, anyhow::Error> {

	let config = config.clone();

	tracing_subscriber::fmt::init();
	
	let mut app = Router::new();

	app = app.layer(CompressionLayer::new().gzip(true));

	let root_path = if let Some(root) = &args.root {

		if root.starts_with("%") {
			expand_env_vars(root)?
		}
		else {
			shellexpand::full(&root)?.into()
		}
	}
	else {
		".".into()
	};

	let root_folder = ServeDir::new(root_path).precompressed_gzip().precompressed_br()
	.not_found_service(ServeFile::new("index.html").precompressed_br().precompressed_gzip());
	
	app = app.fallback_service(root_folder);

	let server = config.server.clone();
	
	for route in server.route.clone() {

		if route.ifs.is_some() {

			let ifs = route.ifs.unwrap();
			
			let path: String = if ifs.starts_with("%") {
				expand_env_vars(&ifs)?
			}
			else {
				shellexpand::env(&ifs)?.into()
			};

			let dir = ServeDir::new(path);
			app = app.nest_service(&route.path, dir);

			if server.cache {
				app = app.layer(from_fn(set_static_cache_control));
			}
		}
		else {

			let route_address = if let Some(route_address) = route.address {
				route_address
			}
			else {
				server.address.clone()
			};

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

	app = if let Some(val) = server.https && val {
		let proxy = axum_proxy::builder_https(server.address.clone())?.build(AppendPrefix(""));
		app.route_service("/out/{*path}", proxy)
	}
	else {
		let proxy = axum_proxy::builder_http(server.address.clone())?.build(AppendPrefix(""));
		app.route_service("/out/{*path}", proxy)
	};

	app = app.route("/plugins/{*path}", get(plugins_get));

	app = app.route("/plugins/{*path}", post(plugins_post));

	app = app.layer(from_fn(log_request));

	app = app.route_layer(from_fn_with_state(config.clone(), set_headers));

	Ok(app)

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

async fn set_static_cache_control(request: Request, next: Next) -> Response {

	let mut response = next.run(request).await;

	response.headers_mut().insert(
		header::CACHE_CONTROL,
		HeaderValue::from_static("public, max-age=3600"),
	);

	response
}