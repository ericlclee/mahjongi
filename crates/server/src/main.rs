use mahjongi_server::server::{Server, CLAIM_HOLD, CLAIM_WINDOW};
use mahjongi_server::{app, spawn_sweeper, spawn_timer};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Seconds from an environment variable, or the default.
fn secs(var: &str, default: Duration) -> Duration {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .map(Duration::from_secs_f64)
        .unwrap_or(default)
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let web_dir = PathBuf::from(std::env::var("WEB_DIR").unwrap_or_else(|_| "web".to_string()));

    // CLAIM_WINDOW_SECS=0 turns the pause after discards off.
    let server = Server::with_claim_window(
        secs("CLAIM_WINDOW_SECS", CLAIM_WINDOW),
        secs("CLAIM_HOLD_SECS", CLAIM_HOLD),
    )
    .with_bot_delay(secs("BOT_DELAY_SECS", Duration::from_millis(800)));
    let shared = Arc::new(Mutex::new(server));
    spawn_sweeper(shared.clone());
    spawn_timer(shared.clone());

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("could not bind the port");
    println!("mahjongi listening on http://0.0.0.0:{port} (web files from {web_dir:?})");
    axum::serve(listener, app(shared, web_dir))
        .await
        .expect("server error");
}
