use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "rhof-cli")]
#[command(about = "RHOF command-line interface")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Sync,
    Migrate,
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Sync) {
        Commands::Sync => {
            let summary = rhof_sync::run_sync_once_from_env().await?;
            println!(
                "sync complete: run_id={} sources={} drafts={} reports={}",
                summary.run_id, summary.enabled_sources, summary.parsed_drafts, summary.reports_dir
            );
        }
        Commands::Migrate => {
            eprintln!("migrate command scaffolded; sqlx wiring lands in later prompts");
        }
        Commands::Serve => {
            eprintln!("serve command scaffolded; web wiring lands in later prompts");
        }
    }

    Ok(())
}
