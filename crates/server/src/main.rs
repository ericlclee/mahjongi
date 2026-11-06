use mahjongi_server::{app, server::Server, spawn_sweeper};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let web_dir = PathBuf::from(std::env::var("WEB_DIR").unwrap_or_else(|_| "web".to_string()));

    let shared = Arc::new(Mutex::new(Server::new()));
    spawn_sweeper(shared.clone());

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("could not bind the port");
    println!("mahjongi listening on http://0.0.0.0:{port} (web files from {web_dir:?})");
    axum::serve(listener, app(shared, web_dir))
        .await
        .expect("server error");
}
