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
    Report {
        #[command(subcommand)]
        command: ReportCommands,
    },
    Migrate,
    Serve,
}

#[derive(Debug, Subcommand)]
enum ReportCommands {
    Daily {
        #[arg(long, default_value_t = 3)]
        runs: usize,
    },
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
            println!("parquet manifest: {}", summary.parquet_manifest);
        }
        Commands::Report { command } => match command {
            ReportCommands::Daily { runs } => {
                let markdown = rhof_sync::report_daily_markdown(runs, None)?;
                println!("{markdown}");
            }
        },
        Commands::Migrate => {
            eprintln!("migrate command scaffolded; sqlx wiring lands in later prompts");
        }
        Commands::Serve => {
            eprintln!("serve command scaffolded; web wiring lands in later prompts");
        }
    }

    Ok(())
}
