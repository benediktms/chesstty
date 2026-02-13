mod app;
mod chess;
mod engine;
mod ui;
mod util;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("ChessTTY - Terminal Chess Application");
    println!("Press 'q' to quit");

    ui::run_app().await?;

    Ok(())
}
