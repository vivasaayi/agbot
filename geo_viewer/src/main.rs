mod app;
mod http;
mod plugins;
mod state;

fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--check-connection") {
        let base =
            std::env::var("GEO_HUB_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
        let token = std::env::var("GEO_VIEWER_SESSION_TOKEN")
            .map_err(|_| anyhow::anyhow!("a viewer session is required; use scripts/viewer"))?;
        http::check_connection(&base, &token)?;
        println!("Authenticated Geo Hub connection succeeded.");
        return Ok(());
    }
    app::run()
}
