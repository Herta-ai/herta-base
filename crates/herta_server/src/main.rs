use std::{path::PathBuf, sync::Arc};

use clap::{Parser, Subcommand};
mod db_log_layer;
mod extensions;
mod ui;

use herta_api::{ApiState, build_router_with_logger};
use herta_core::HbConfig;
use herta_db::{DbClient, log_channel, spawn_log_worker};
use salvo::prelude::*;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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
            let config = HbConfig::load_with_overrides(cli.config.as_deref(), |config| {
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
            })?;

            let persist_logs = config.log.server_persist_enabled || config.log.http_persist_enabled;
            let (log_sender, log_receiver) = if persist_logs {
                let (sender, receiver) = log_channel();
                (Some(sender), Some(receiver))
            } else {
                (None, None)
            };
            init_logging(&config, log_sender.clone())?;

            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                engine = %config.database.engine,
                "starting HertaBase"
            );
            let db = DbClient::init(&config).await?;
            let _log_worker = log_receiver.map(|receiver| spawn_log_worker(db.clone(), receiver));
            let request_logger = if config.log.http_persist_enabled {
                log_sender
                    .clone()
                    .map(herta_api::handlers::logging::RequestLogger::new)
            } else {
                None
            };
            let mut state = ApiState::new(db, config.clone()).await?;
            let mut extensions = extensions::RuntimeServices::start(&mut state).await?;
            let messages = state.messages.clone();
            let router = build_router_with_logger(request_logger).push(ui::router());
            let service = Service::new(router)
                .hoop(affix_state::inject(Arc::new(state)))
                .hoop(herta_api::handlers::extensions::dispatch);
            let address = format!("{}:{}", config.server.host, config.server.port);
            let acceptor = TcpListener::new(address.clone()).try_bind().await?;
            if let Some(runtime) = &mut extensions {
                runtime.lifecycle("serve").await?;
                runtime.start_scheduled_tasks();
            }
            tracing::info!(%address, "server listening");
            tracing::info!(url = %format!("http://{address}/webui/"), "Admin UI");
            tracing::info!(url = %format!("http://{address}/swagger-ui/"), "Swagger UI");
            let server = Server::new(acceptor);
            let handle = server.handle();
            let serving = server.try_serve(service);
            tokio::pin!(serving);
            let result = tokio::select! {
                result = &mut serving => result,
                signal = extensions::shutdown_signal() => {
                    messages.shutdown();
                    if let Some(runtime) = &mut extensions { runtime.stop_background(); }
                    handle.stop_graceful(Some(std::time::Duration::from_millis(config.jsvm.shutdown_timeout_ms)));
                    let served = serving.await;
                    signal.and(served)
                }
            };
            messages.shutdown();
            if let Some(runtime) = extensions {
                runtime.shutdown().await?;
            }
            result?;
        }
        Command::Version => println!("hertabase v{}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}

fn init_logging(config: &HbConfig, log_sender: Option<herta_db::LogSender>) -> anyhow::Result<()> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log.level));
    let db_layer = if config.log.server_persist_enabled {
        log_sender
            .map(|sender| db_log_layer::DbLogLayer::new(sender, &config.log.server_persist_level))
    } else {
        None
    };
    if config.log.format == "json" {
        let fmt_layer = tracing_subscriber::fmt::layer().json().with_filter(filter);
        tracing_subscriber::registry()
            .with(fmt_layer)
            .with(db_layer)
            .try_init()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    } else {
        let fmt_layer = tracing_subscriber::fmt::layer().with_filter(filter);
        tracing_subscriber::registry()
            .with(fmt_layer)
            .with(db_layer)
            .try_init()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    }
    Ok(())
}
