use std::path::PathBuf;

use clap::{Parser, Subcommand};
use herta_api::{ApiState, build_router};
use herta_core::HbConfig;
use herta_db::DbClient;
use salvo::prelude::*;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "hertabase",
    version,
    about = "Single-binary backend as a service"
)]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Serve {
        #[arg(short = 'H', long)]
        host: Option<String>,
        #[arg(short, long)]
        port: Option<u16>,
        #[arg(long)]
        data_dir: Option<String>,
        #[arg(long)]
        hooks_dir: Option<String>,
        #[arg(long)]
        db_engine: Option<String>,
        #[arg(long)]
        dev: bool,
    },
    Version,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve {
        host: None,
        port: None,
        data_dir: None,
        hooks_dir: None,
        db_engine: None,
        dev: false,
    }) {
        Command::Serve {
            host,
            port,
            data_dir,
            hooks_dir,
            db_engine,
            dev,
        } => {
            let mut config = HbConfig::load(cli.config.as_deref())?;
            if let Some(host) = host {
                config.server.host = host;
            }
            if let Some(port) = port {
                config.server.port = port;
            }
            if let Some(data_dir) = data_dir {
                config.paths.data_dir = data_dir;
            }
            if let Some(hooks_dir) = hooks_dir {
                config.paths.hooks_dir = hooks_dir;
            }
            if let Some(db_engine) = db_engine {
                config.database.engine = db_engine;
            }
            if dev {
                config.server.dev_mode = true;
            }
            config.validate()?;
            init_logging(&config)?;

            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                engine = %config.database.engine,
                "starting HertaBase"
            );
            let db = DbClient::init(&config).await?;
            let state = ApiState::new(db, config.clone()).await?;
            let service = Service::new(build_router()).hoop(affix_state::inject(state));
            let address = format!("{}:{}", config.server.host, config.server.port);
            let acceptor = TcpListener::new(address.clone()).bind().await;
            tracing::info!(%address, "server listening");
            tracing::info!(url = %format!("http://{address}/swagger-ui/"), "Swagger UI");
            Server::new(acceptor).serve(service).await;
        }
        Command::Version => println!("hertabase v{}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}

fn init_logging(config: &HbConfig) -> anyhow::Result<()> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log.level));
    if config.log.format == "json" {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .try_init()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .try_init()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    }
    Ok(())
}
