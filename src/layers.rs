use std::{borrow::Cow, task::{Context, Poll}};

use axum::{extract::Request, http::Uri};
use axum_proxy::PathRewriter;
use tower::{Layer, Service};

#[derive(Clone)]
pub struct RewriteLayer;

impl<S> Layer<S> for RewriteLayer{
    type Service = RewriteService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RewriteService {
            inner
        }
    }
}

#[derive(Clone)]
pub struct RewriteService<S> {
    pub inner: S
}

impl<S, B> Service<Request<B>> for RewriteService<S>
where
     S: Service<Request<B>>
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

