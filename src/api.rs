use std::path::PathBuf;
use std::str::FromStr;

use anyhow::anyhow;
use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::{Next, from_fn, from_fn_with_state};
use axum::response::Response;
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
use crate::{
    config::WebConfig,
    plugins::{plugins_get, plugins_post},
};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use notify::{RecursiveMode, Watcher};
use tokio::sync::broadcast;

use std::collections::HashMap;
use std::time::SystemTime;

pub async fn init(args: &Args) -> Result<(Router, TcpListener), anyhow::Error> {
    let root = if let Some(path) = &args.root {
        if path.starts_with("%") {
            expand_env_vars(path)?
        } else if path.starts_with("~") {
            shellexpand::full(&path)?.into()
        } else {
            shellexpand::env(&path)?.into()
        }
    } else {
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
        } else {
            root_path.join("webconfig.toml")
        };

        if file_path.exists() {
            let _ = std::env::set_current_dir(&root_path);
        } else {
            return Err(anyhow!("Toml not found mut be place in root of server"));
        }

        OpenOptions::new().read(true).open(file_path).await.unwrap()
    } else {
        return Err(anyhow!("Invalid path"));
    };

    let mut buffer = String::from("");

    file.read_to_string(&mut buffer).await?;

    let config: WebConfig = toml::from_str(&buffer)?;

    let config = config.clone();

    tracing_subscriber::fmt::init();

    let (tx, _rx) = broadcast::channel::<String>(100);

    let mut app = Router::new();

    app = app.layer(CompressionLayer::new().gzip(true));

    let server = config.server.clone();

    let mut root_used = false;

    if args.verbose {
        info!("Server Root: {:?}", &root_path);
    }

    if let Some(routes) = server.route {
        for route in routes.iter() {
            if route.ifs.is_some() {
                let ifs = route.ifs.clone().unwrap();

                let path: String = if ifs.starts_with("%") {
                    expand_env_vars(&ifs)?
                } else {
                    shellexpand::full(&ifs)?.into()
                };

                let dir = ServeDir::new(&path);
                app = app.nest_service(&route.path, dir);

                if args.verbose {
                    info!("Route: {}", &route.path);
                    info!("IFS path: {}", &path);
                }

                if let Some(true) = route.watch {
                    let tx = tx.clone();
                    let watch_path = std::path::Path::new(&path).canonicalize().unwrap();
                    let web_base = if route.path == "/" {
                        "".to_string()
                    } else {
                        route.path.clone()
                    };

                    tokio::spawn(async move {
                        let (watch_tx, mut watch_rx) = tokio::sync::mpsc::channel(1);

                        let mut watcher = notify::recommended_watcher(move |res| {
                            if let Ok(event) = res {
                                let _ = watch_tx.blocking_send(event);
                            }
                        })
                        .unwrap();

                        watcher
                            .watch(&watch_path, RecursiveMode::Recursive)
                            .unwrap();

                        let mut last_sent: HashMap<std::path::PathBuf, SystemTime> = HashMap::new();

                        loop {
                            tokio::select! {
                                Some(event) = watch_rx.recv() => {
                                    match event.kind {
                                        notify::EventKind::Remove(_) => {},
                                        notify::EventKind::Modify(kind) => {
                                            match kind {
                                                notify::event::ModifyKind::Data(_) | notify::event::ModifyKind::Any => {
                                                    for path in event.paths {
                                                        if path.is_file() && let Ok(meta) = std::fs::metadata(&path) && let Ok(modified) = meta.modified() {

                                                            let should_send = match last_sent.get(&path) {
                                                                Some(prev) => *prev != modified,
                                                                None => true,
                                                            };

                                                            if should_send {
                                                                last_sent.insert(path.clone(), modified);

                                                                if let Ok(abs_path) = path.canonicalize()
                                                                    && let Ok(rel_path) = abs_path.strip_prefix(&watch_path)
                                                                {
                                                                    let web_path = format!("{}/{}", web_base, rel_path.to_string_lossy().replace('\\', "/"));
                                                                    let web_path = web_path.replace("//", "/");

                                                                    if let Ok(content) = std::fs::read_to_string(&abs_path)
                                                                    {
                                                                        let payload = serde_json::json!({
                                                                            "path": web_path,
                                                                            "content": content
                                                                        });
                                                                        let _ = tx.send(payload.to_string());
                                                                    }
                                                                }
                                                            }

                                                        }
                                                    }
                                                },
                                                _=> continue,
                                            }

                                        }
                                        _ => {}
                                    }
                                }
                                _ = shutdown_signal() => {
                                    break;
                                }
                                else => break,
                            }
                        }
                    });

                    if args.verbose {
                        info!("Watching for changes on: {}", &path);
                    }
                }
            } else {
                let route_address = if let Some(route_address) = &route.address {
                    route_address.to_string()
                } else {
                    server.address.clone()
                };

                if route.path == "/" {
                    root_used = true;
                }

                let https = if let Some(val) = route.https {
                    val
                } else {
                    let mut result = false;

                    if let Some(server_val) = server.https {
                        result = server_val
                    }

                    result
                };

                app = if https {
                    let proxy = if let Some(val) = route.strip
                        && val
                    {
                        axum_proxy::builder_https(route_address)?
                            .build(TrimToWildcard(route.path.clone()))
                    } else {
                        axum_proxy::builder_https(route_address)?.build(TrimToWildcard("".into()))
                    };
                    app.route_service(&route.path, proxy)
                } else {
                    let proxy = axum_proxy::builder_http(route_address)?.build(AppendPrefix(""));
                    app.route_service(&route.path, proxy)
                };
            }
        }
    }

    if !server.address.is_empty() {
        app = if let Some(val) = server.https
            && val
        {
            let proxy =
                axum_proxy::builder_https(server.address.clone())?.build(TrimPrefix("/out"));
            app.route_service("/out/{*path}", proxy)
        } else {
            let proxy = axum_proxy::builder_http(server.address.clone())?.build(TrimPrefix("/out"));
            app.route_service("/out/{*path}", proxy)
        };
    }

    app = app.route("/plugins/{*path}", get(plugins_get));

    app = app.route("/plugins/{*path}", post(plugins_post));

    app = app.route(
        "/ws",
        get({
            let tx = tx.clone();
            move |ws: WebSocketUpgrade| {
                let rx = tx.subscribe();
                async move { ws.on_upgrade(|socket| handle_hotreload_socket(socket, rx)) }
            }
        }),
    );

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

async fn set_headers(State(state): State<WebConfig>, request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let server = state.server;

    let headers = response.headers_mut();

    if let Some(route_headers) = server.headers {
        for h in route_headers {
            headers.insert(
                HeaderName::from_str(&h.key).unwrap(),
                HeaderValue::from_str(&h.value).unwrap(),
            );
        }
    }

    response
}

async fn handle_hotreload_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
            msg = socket.recv() => {
                if let Some(Ok(Message::Close(_))) | None = msg {
                    break;
                }
            }
            _ = shutdown_signal() => {
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
        }
    }
}

pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(windows)]
    let terminate = async {
        let mut ctrl_close =
            tokio::signal::windows::ctrl_close().expect("failed to install CTRL_CLOSE handler");
        ctrl_close.recv().await;
    };

    #[cfg(not(any(unix, windows)))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
