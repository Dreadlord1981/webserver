use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Server {
    pub address: String,
    pub port: i32,
    pub plugins: Option<String>,
    pub route: Option<Vec<WebRoute>>,
    pub headers: Option<Vec<ServerHeader>>,
    pub https: Option<bool>,
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
    pub strip: Option<bool>,
    pub watch: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ServerHeader {
    pub key: String,
    pub value: String,
}
