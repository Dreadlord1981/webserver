use std::{borrow::Cow, task::{Context, Poll}};

use axum::{extract::Request, http::Uri};
use axum_proxy::PathRewriter;
use tower::{Layer, Service};

#[derive(Clone)]
pub struct RewriteLayer<T> 
where T: Clone{
    pub state: T,
}

impl<S, T> Layer<S> for RewriteLayer<T>
where T: Clone {
    type Service = RewriteService<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        RewriteService {
            inner,
            state: self.state.clone(),
        }
    }
}

#[derive(Clone)]
pub struct RewriteService<S, T> {
    inner: S,
    state: T,
}

impl<S, B, T> Service<Request<B>> for RewriteService<S, T>
where
     S: Service<Request<B>>,
	 T: Clone + Send + Sync + 'static
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrimToWildcard(pub String);

impl PathRewriter for TrimToWildcard {
	fn rewrite<'a>(&'a mut self, path: &'a str) -> Cow<'a, str> {

		let wildcard = self.0.clone();

		let result = if wildcard.contains("/{*") {

			let index = wildcard.rfind("/{*").unwrap();

			&path[index..]
		}
		else if path.contains(&wildcard) {

			path.strip_prefix(&wildcard).unwrap()
		}
		else {
			path
		};

		result.into()
	}
}

