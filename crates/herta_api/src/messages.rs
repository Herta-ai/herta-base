//! Bounded, non-durable application bus. Tokens and identity stay in Rust.
use herta_auth::{AuthIdentity, AuthService};
use herta_core::{
    HbError, HbResult,
    jsvm::JsRealtimeConfig,
    messages::{Audience, Message, PublishReceipt, PublishRequest, validate_topic},
};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct RealtimeBus {
    inner: Arc<Inner>,
}
struct Inner {
    auth: AuthService,
    config: JsRealtimeConfig,
    state: Mutex<State>,
    stopped: CancellationToken,
}
#[derive(Default)]
struct State {
    connections: HashMap<String, Arc<Connection>>,
    publications: VecDeque<Instant>,
}
struct Connection {
    id: String,
    topic: String,
    token: String,
    expires_at: u64,
    sender: mpsc::Sender<Pending>,
    bytes: Arc<Semaphore>,
    closed: CancellationToken,
}
struct Pending {
    message: Arc<Message>,
    audience: Arc<Audience>,
    _bytes: OwnedSemaphorePermit,
}
pub struct MessageSubscription {
    bus: RealtimeBus,
    connection: Arc<Connection>,
    receiver: mpsc::Receiver<Pending>,
}
impl RealtimeBus {
    pub fn new(auth: AuthService, config: JsRealtimeConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                auth,
                config,
                state: Mutex::new(State::default()),
                stopped: CancellationToken::new(),
            }),
        }
    }
    pub async fn subscribe(&self, topic: String, token: String) -> HbResult<MessageSubscription> {
        if self.inner.stopped.is_cancelled() {
            return Err(HbError::CapabilityUnavailable);
        }
        validate_topic(&topic)?;
        let authentication = self.inner.auth.authenticate_with_expiry(&token).await?;
        if authentication.identity == AuthIdentity::Anonymous {
            return Err(HbError::AuthRequired);
        }
        if authentication.expires_at <= seconds() {
            return Err(HbError::TokenExpired);
        }
        let (sender, receiver) = mpsc::channel(self.inner.config.connection_queue_capacity);
        let connection = Arc::new(Connection {
            id: uuid::Uuid::now_v7().to_string(),
            topic,
            token,
            expires_at: authentication.expires_at,
            sender,
            bytes: Arc::new(Semaphore::new(self.inner.config.connection_queue_bytes)),
            closed: CancellationToken::new(),
        });
        self.inner
            .state
            .lock()
            .map_err(|_| HbError::Internal)?
            .connections
            .insert(connection.id.clone(), connection.clone());
        Ok(MessageSubscription {
            bus: self.clone(),
            connection,
            receiver,
        })
    }
    pub async fn publish(&self, request: PublishRequest) -> HbResult<PublishReceipt> {
        let audience = Arc::new(request.audience(self.inner.config.max_audience)?.clone());
        let message = Arc::new(Message {
            id: uuid::Uuid::now_v7().to_string(),
            topic: request.topic,
            data: request.data,
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        });
        let size = serde_json::to_vec(message.as_ref())
            .map_err(|_| HbError::Internal)?
            .len();
        if size > self.inner.config.max_message_bytes {
            return Err(HbError::PayloadTooLarge);
        }
        if self.inner.stopped.is_cancelled() {
            return Err(HbError::CapabilityUnavailable);
        }
        let targets: Vec<_> = {
            let mut state = self.inner.state.lock().map_err(|_| HbError::Internal)?;
            let now = Instant::now();
            while state
                .publications
                .front()
                .is_some_and(|time| now.duration_since(*time) >= Duration::from_secs(1))
            {
                state.publications.pop_front();
            }
            if state.publications.len() >= self.inner.config.publish_per_second {
                return Err(HbError::RateLimited);
            }
            state.publications.push_back(now);
            state
                .connections
                .values()
                .filter(|connection| connection.topic == message.topic)
                .cloned()
                .collect()
        };
        let mut receipt = PublishReceipt::default();
        for connection in targets {
            if connection.closed.is_cancelled() {
                continue;
            }
            let identity = match self.identity(&connection).await {
                Ok(identity) => identity,
                Err(_) => {
                    connection.closed.cancel();
                    receipt.dropped += 1;
                    continue;
                }
            };
            if !matches(&audience, &identity, &connection.id) {
                continue;
            }
            let bytes = connection.bytes.clone().try_acquire_many_owned(size as u32);
            let sent = bytes.is_ok_and(|bytes| {
                connection
                    .sender
                    .try_send(Pending {
                        message: message.clone(),
                        audience: audience.clone(),
                        _bytes: bytes,
                    })
                    .is_ok()
            });
            if sent {
                receipt.queued += 1;
            } else {
                connection.closed.cancel();
                receipt.dropped += 1;
            }
        }
        Ok(receipt)
    }
    async fn identity(&self, connection: &Connection) -> HbResult<AuthIdentity> {
        if seconds() >= connection.expires_at {
            return Err(HbError::TokenExpired);
        }
        self.inner.auth.authenticate(&connection.token).await
    }
    pub fn shutdown(&self) {
        self.inner.stopped.cancel();
    }
}
impl MessageSubscription {
    pub fn id(&self) -> &str {
        &self.connection.id
    }
    pub fn topic(&self) -> &str {
        &self.connection.topic
    }
    pub async fn check(&self) -> HbResult<()> {
        self.bus.identity(&self.connection).await.map(|_| ())
    }
    pub async fn next(&mut self) -> HbResult<Option<Arc<Message>>> {
        loop {
            let delay = Duration::from_secs(self.connection.expires_at.saturating_sub(seconds()));
            let pending = tokio::select! {
                biased;
                _ = self.bus.inner.stopped.cancelled() => return Ok(None),
                _ = self.connection.closed.cancelled() => return Ok(None),
                _ = tokio::time::sleep(delay) => return Err(HbError::TokenExpired),
                value = self.receiver.recv() => value,
            };
            let Some(pending) = pending else {
                return Ok(None);
            };
            // Recheck after dequeue: revocation or role changes while queued cannot leak data.
            let identity = self.bus.identity(&self.connection).await?;
            if matches(&pending.audience, &identity, &self.connection.id) {
                return Ok(Some(pending.message));
            }
        }
    }
}
impl Drop for MessageSubscription {
    fn drop(&mut self) {
        self.connection.closed.cancel();
        if let Ok(mut state) = self.bus.inner.state.lock() {
            state.connections.remove(&self.connection.id);
        }
    }
}
fn seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn matches(audience: &Audience, identity: &AuthIdentity, connection: &str) -> bool {
    let (collection, id, role) = match identity {
        AuthIdentity::Anonymous => return false,
        AuthIdentity::User {
            collection,
            id,
            role,
            ..
        } => (collection.as_str(), id.as_str(), role.as_str()),
        AuthIdentity::Admin { id, role, .. } => ("_admins", id.as_str(), role.as_str()),
    };
    match audience {
        Audience::Connections(ids) => ids.iter().any(|id| id == connection),
        Audience::Roles(roles) => roles
            .iter()
            .any(|target| target.collection == collection && target.role == role),
        Audience::Users(users) => users.iter().any(|target| {
            target.collection == collection
                && (target.id == id
                    || id
                        .strip_prefix(collection)
                        .and_then(|id| id.strip_prefix(':'))
                        == Some(target.id.as_str()))
        }),
    }
}
