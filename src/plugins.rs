use std::collections::HashMap;

use anyhow::anyhow;
use axum::{Json, body::Body, extract::Request, response::IntoResponse};
use serde_json::Value;

use crate::errors::AppError;

type Pgm = fn(payload: Value) -> Result<Value, Box<dyn std::error::Error>>;

#[allow(unused_assignments)]
pub async fn plugins_get(
	request: Request
) -> impl IntoResponse  {

	let mut value =  Value::Null;

	let uri = request.uri().clone();
	let path = uri.path();
	let query_result = uri.query();

	let mut map = HashMap::<String, Value>::new();

	if let Some(query) = query_result {

		let params = query.split("&");
		
		for p in params {

			let mut split = p.split("=");

			let property = split.next().unwrap();
			let value = split.next().unwrap();

			let val = if let Ok(val) = value.parse::<i64>() {
				serde_json::to_value(val).unwrap()
			}
			else if let Ok(val) = value.parse::<bool>()  {
				serde_json::to_value(val).unwrap()
			}
			else {
				serde_json::to_value(value).unwrap()
			};

			map.insert(property.into(), val);
		}
	}

	let payload = serde_json::to_value(&map).unwrap_or(Value::Null);

	let split = path.split("/");
	let mut plugin = split.last().unwrap().to_string();

	if plugin.contains(".") {

		let cloned = plugin.clone();

		let mut plugin_split = cloned.split(".").clone();
		plugin = plugin_split.next().unwrap().to_string();
		let method = plugin_split.last().unwrap().to_string();

		unsafe {

			let mut current_dir = std::env::current_dir()?;
			let mut plugin_path = current_dir.join("plugins");

			if !plugin_path.exists() {

				current_dir = std::env::current_exe()?;
				current_dir.pop();

				plugin_path = current_dir.join("plugins");
			}

			if plugin_path.exists() {

				let lib_path = plugin_path.join(format!("{plugin}.dll"));

				let lib_result = libloading::Library::new(lib_path);

				if let Ok(lib) = lib_result {

					let func_result: Result<libloading::Symbol<Pgm>, libloading::Error> = lib.get(method.to_string());

					if let Ok(func) = func_result {

						let call_result = func(payload);

						if let Ok(data) = call_result {
							value = data;
						}
						else {

							let err = call_result.err().unwrap();
							let error = format!("{}", err);

							return Err(AppError::from(anyhow!(error)));
						}
						
					}
					else {
						return Err(AppError::from(anyhow!("Error invalid method not found")));
					}
				}
				else {
					return Err(AppError::from(anyhow!("Invalid library not found")));
				}
			}
			else {
				return Err(AppError::from(anyhow!("Invalid path not found")));
			}
		}
		
	}
	else {
		return Err(AppError::from(anyhow!("Error missing parameter program to call")));
	}
	
	Ok(Json(value))
}


#[allow(unused_assignments)]
pub async fn plugins_post(
	request: Request<Body>
) -> impl IntoResponse {

	let mut value =  Value::Null;

	let uri = request.uri().clone();
	let path = uri.path();
	let body = request.into_body();

	let bytes = axum::body::to_bytes(body, 8192).await?;

	let payload: Value = serde_json::from_str(&String::from_utf8_lossy(&bytes))?;

	let split = path.split("/");
	let mut plugin = split.last().unwrap().to_string();

	if plugin.contains(".") {

		let cloned = plugin.clone();

		let mut plugin_split = cloned.split(".").clone();
		plugin = plugin_split.next().unwrap().to_string();
		let method = plugin_split.last().unwrap().to_string();

		unsafe {

			let mut current_dir = std::env::current_dir()?;
			let mut plugin_path = current_dir.join("plugins");

			if !plugin_path.exists() {

				current_dir = std::env::current_exe()?;
				current_dir.pop();

				plugin_path = current_dir.join("plugins");
			}


			if plugin_path.exists() {

				let lib_result = libloading::Library::new(format!("plugins/{plugin}.dll"));

				if let Ok(lib) = lib_result {

					let func_result: Result<libloading::Symbol<Pgm>, libloading::Error> = lib.get(method.to_string());
					
					if let Ok(func) = func_result {

						let call_result = func(payload);

						if let Ok(data) = call_result {
							value = data;
						}
						else {

							let err = call_result.err().unwrap();
							let error = format!("{}", err);

							return Err(AppError::from(anyhow!(error)));
						}
						
					}
					else {

						return Err(AppError::from(anyhow!("Error invalid method not found")));
					}
				}
				else {
					return Err(AppError::from(anyhow!("Invalid plugin not found")));
				}
			}
			else {

				return Err(AppError::from(anyhow!("Invalid library not found")));
			}
		}
	}
	else {
		return Err(AppError::from(anyhow!("Error missing parameter program to call")));
	}
	
	Ok(Json(value))
}