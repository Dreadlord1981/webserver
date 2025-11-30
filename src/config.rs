use std::{borrow::Cow, convert::Infallible, sync::Arc};

use axum::{body::Body, extract::Request, http::{Response, Uri}};
use axum_proxy::PathRewriter;
use serde::Deserialize;
use tower::Service;


#[derive(Deserialize, Debug, Clone)]
pub struct Server {
	pub address: String,
	pub port: i32,
	pub cache: bool,
	pub plugins: Option<String>,
	pub route: Vec<WebRoute>
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
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutRewritter<'a>(pub &'a str);

impl PathRewriter for OutRewritter<'_> {
    fn rewrite<'a>(&mut self, path: &'a str) -> Cow<'a, str> {
		
		let result = path.strip_prefix("/out").unwrap_or("").to_string();

		result.into()
    }
}

#[derive(Clone)]
pub struct RewriteWithState<S, T> {
    pub inner: S,
    pub state: Arc<T>,
}

impl<S, T> Service<Request<Body>> for RewriteWithState<S, T>
where
    S: Service<Request<Body>, Response=Response<Body>, Error=Infallible> + Clone + Send + 'static,
    T: Send + Sync + 'static,
{
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        // Rewrite URI
         let orig = req.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("");
	
		if orig.contains(".aspx") {

			let replaced = format!("/out{orig}");

			let new_uri = replaced.parse::<Uri>().unwrap();

			*req.uri_mut() = new_uri;
		}

        // Inject real state
        req.extensions_mut().insert(self.state.clone());

        self.inner.call(req)
    }
}