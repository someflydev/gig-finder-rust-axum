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
    NewAdapter {
        source_id: String,
    },
    Seed,
    Debug,
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
        Commands::NewAdapter { source_id } => {
            let created = rhof_adapters::generate_adapter_scaffold(".", &source_id)?;
            println!("generated adapter scaffold for `{}`", source_id);
            for path in created {
                println!("- {}", path.display());
            }
        }
        Commands::Seed => {
            let summary = rhof_sync::seed_from_fixtures_from_env().await?;
            println!(
                "seed complete (fixture-derived): run_id={} artifacts={} drafts={} reports={}",
                summary.run_id, summary.fetched_artifacts, summary.parsed_drafts, summary.reports_dir
            );
            println!("parquet manifest: {}", summary.parquet_manifest);
        }
        Commands::Debug => {
            let info = rhof_sync::debug_summary_from_env()?;
            println!("{info}");
        }
        Commands::Migrate => {
            rhof_sync::apply_migrations_from_env().await?;
            println!("migrations applied");
        }
        Commands::Serve => {
            rhof_web::serve_from_env().await?;
        }
    }

    Ok(())
}
