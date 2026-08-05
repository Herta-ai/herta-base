use herta_db::{LogEntry, LogSender, LogType};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

const FILTERED_TARGETS: &[&str] = &[
    "surrealdb",
    "surreal",
    "herta_db::log",
    "tokio",
    "hyper",
    "tower",
    "mio",
];

pub struct DbLogLayer {
    sender: LogSender,
    min_level: Level,
}

impl DbLogLayer {
    pub fn new(sender: LogSender, level: &str) -> Self {
        Self {
            sender,
            min_level: parse_level(level),
        }
    }
}

impl<S> Layer<S> for DbLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if level_rank(*event.metadata().level()) < level_rank(self.min_level) {
            return;
        }
        let target = event.metadata().target();
        if FILTERED_TARGETS
            .iter()
            .any(|prefix| target.starts_with(prefix))
        {
            return;
        }

        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let entry = LogEntry {
            log_type: LogType::Server,
            level: event.metadata().level().to_string().to_lowercase(),
            message: visitor.message,
            target: target.to_string(),
            method: None,
            path: None,
            status_code: None,
            referer: None,
            remote_ip: None,
            user_agent: None,
            auth_type: None,
            user_id: None,
            user_collection: None,
        };
        let _ = self.sender.try_send(entry);
    }
}

fn parse_level(level: &str) -> Level {
    match level {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    }
}

fn level_rank(level: Level) -> u8 {
    match level {
        Level::ERROR => 4,
        Level::WARN => 3,
        Level::INFO => 2,
        Level::DEBUG => 1,
        Level::TRACE => 0,
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else if self.message.is_empty() {
            self.message = format!("{} = {value:?}", field.name());
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use herta_db::log_channel;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn info_threshold_keeps_info_warn_and_error() {
        let (sender, mut receiver) = log_channel();
        let subscriber = tracing_subscriber::registry().with(DbLogLayer::new(sender, "info"));
        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!("ignored");
            tracing::info!("included info");
            tracing::warn!("included warn");
            tracing::error!("included error");
        });

        let levels: Vec<String> = std::iter::from_fn(|| receiver.try_recv().ok())
            .map(|entry| entry.level)
            .collect();
        assert_eq!(levels, ["info", "warn", "error"]);
    }
}
