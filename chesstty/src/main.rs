use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "chesstty", about = "Chess TUI with integrated engine analysis")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Engine {
        #[command(subcommand)]
        action: EngineAction,
    },
}

#[derive(Subcommand)]
enum EngineAction {
    Stop {
        #[arg(short, long)]
        force: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Engine { action }) => match action {
            EngineAction::Stop { force } => {
                println!("Stopping server (force: {})", force);
            }
        },
        None => {
            println!("Starting ChessTTY...");
        }
    }
}
