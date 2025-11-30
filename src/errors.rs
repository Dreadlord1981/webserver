use axum::{http::StatusCode, response::{IntoResponse, Response}};
use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorMessage {
	pub success: bool,
	pub message: String
}

pub struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {

    fn into_response(self) -> Response {

		let error = ErrorMessage {
			success: false,
			message: format!("{}", self.0)
		};

		let data = serde_json::to_string_pretty(&error).unwrap();

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            data
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
