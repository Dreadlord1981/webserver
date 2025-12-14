
use serde::Deserialize;


#[derive(Deserialize, Debug, Clone)]
pub struct Server {
	pub address: String,
	pub port: i32,
	pub cache: bool,
	pub plugins: Option<String>,
	pub route: Vec<WebRoute>,
	pub headers: Option<Vec<RouteHeader>>,
	pub https: Option<bool>
}

#[derive(Deserialize, Debug, Clone)]
pub struct WebConfig {
	pub server: Server,
}

#[derive(Deserialize, Debug, Clone)]
pub struct WebRoute {
	pub path: String,
	pub ifs: Option<String>,
	pub address: Option<String>,
	pub https: Option<bool>,
	pub strip: Option<bool>
}

#[derive(Deserialize, Debug, Clone)]
pub struct RouteHeader {
	pub key: String,
	pub value: String
}