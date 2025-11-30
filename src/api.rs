
use axum::http::{HeaderValue, header};
use axum::response::Response;
use axum::{extract::Request, http::Uri};
use axum::middleware::{Next, from_fn};
use axum::Router;
use axum::routing::{get, post};
use axum_proxy::AppendPrefix;
use expand_env_vars::expand_env_vars;
use tower_http::{compression::CompressionLayer, services::ServeDir};
use tracing::info;

use crate::cli::Args;
use crate::config::OutRewritter;
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
			shellexpand::env(&root)?.into()
		}
	}
	else {
		".".into()
	};

	let root_folder = ServeDir::new(root_path).precompressed_gzip().precompressed_br();
	
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

			let host1 = axum_proxy::builder_http(route_address)?;
			let proxy = host1.build(AppendPrefix(""));
			 
			app = app.route_service(&route.path, proxy);
		}
	}

	let host1 = axum_proxy::builder_http(server.address.clone())?;
	let proxy = host1.build(OutRewritter(""));
		
	app = app.route_service("/out/{*path}", proxy);

	app = app.route("/plugins/{*path}", get(plugins_get));

	app = app.route("/plugins/{*path}", post(plugins_post));

	app = app.layer(from_fn(log_request));

	Ok(app)

}

async fn log_request(req: Request, next: axum::middleware::Next) -> axum::response::Response {
    // Log the request details
    info!("{} {}", req.method(), req.uri());

    // Continue to the next middleware or handler
    next.run(req).await
}

pub fn rewrite_uri(mut req: Request) -> Request {

    let orig = req.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("");
	
    if orig.contains(".aspx") {

		let replaced = format!("/out{orig}");

		let new_uri = replaced.parse::<Uri>().unwrap();

		*req.uri_mut() = new_uri;
	}
    req
}

async fn set_static_cache_control(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );
    response
}
