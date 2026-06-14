mod app;
mod plugins;
mod state;

fn main() -> anyhow::Result<()> {
    app::run()
}
