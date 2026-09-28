//! Independent transaction ownership. Accepted commands survive waiter cancellation.

use crate::{DbClient, DbSession, schema::database_error};
use futures_util::future::BoxFuture;
use herta_core::{HbError, HbResult, JsError, JsErrorKind};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use surrealdb::{engine::local::Db, method::Transaction};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Pending,
    Committing,
    Committed,
    RolledBack,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationStatus {
    pub operation_id: String,
    pub state: OperationState,
}

/// The credential is returned only to the original caller; only its hash is stored.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionReceipt {
    pub id: String,
    pub operation_id: String,
    pub check_credential: String,
    pub check_url: String,
}

struct Active {
    transaction: Transaction<Db>,
    receipt: TransactionReceipt,
    rollback_only: bool,
}
enum State {
    Idle,
    Active(Active),
    Unknown(TransactionReceipt),
}

struct Inner {
    db: DbClient,
    principal: Option<String>,
    state: Mutex<State>,
    cancellation: CancellationToken,
    executor: tokio::runtime::Handle,
    commit_receipt: std::sync::Mutex<Option<TransactionReceipt>>,
}

/// Contains database commands and ownership only; it never holds a dispatcher.
#[derive(Clone)]
pub struct TransactionOwner(Arc<Inner>);

impl TransactionOwner {
    pub fn new(db: DbClient, principal: Option<String>) -> Self {
        Self(Arc::new(Inner {
            db,
            principal,
            state: Mutex::new(State::Idle),
            cancellation: CancellationToken::new(),
            executor: tokio::runtime::Handle::current(),
            commit_receipt: std::sync::Mutex::new(None),
        }))
    }

    pub fn cancel(&self) {
        self.0.cancellation.cancel();
    }

    pub fn interrupted_commit(&self) -> Option<HbError> {
        self.0.commit_receipt.lock().ok()?.as_ref().map(unknown)
    }

    pub async fn begin(&self) -> HbResult<TransactionReceipt> {
        let inner = self.0.clone();
        self.0
            .executor
            .spawn(async move {
                let mut state = inner.state.lock().await;
                if inner.cancellation.is_cancelled() {
                    return Err(JsErrorKind::Timeout.into());
                }
                if !matches!(*state, State::Idle) {
                    return Err(HbError::Conflict(
                        "a transaction is already active or unresolved".into(),
                    ));
                }
                let id = uuid::Uuid::now_v7().to_string();
                let mut secret = [0u8; 32];
                rand::thread_rng().fill_bytes(&mut secret);
                let credential: String = secret.iter().map(|byte| format!("{byte:02x}")).collect();
                let receipt = TransactionReceipt {
                    id: id.clone(),
                    operation_id: id.clone(),
                    check_credential: credential.clone(),
                    check_url: format!("/api/operations/{id}"),
                };
                inner
                    .db
                    .inner()
                    .query("CREATE ONLY type::record('_operations', $id) CONTENT $entry")
                    .bind(("id", id.clone()))
                    .bind((
                        "entry",
                        json!({"operationId":id,"state":"pending",
                    "principal":inner.principal,"credentialHash":credential_hash(&credential)}),
                    ))
                    .await
                    .map_err(database_error)?
                    .check()
                    .map_err(database_error)?;
                let transaction = match inner.db.inner().clone().begin().await {
                    Ok(transaction) => transaction,
                    Err(error) => {
                        let _ = set_status(
                            &inner.db,
                            &receipt.operation_id,
                            OperationState::RolledBack,
                        )
                        .await;
                        return Err(database_error(error));
                    }
                };
                *state = State::Active(Active {
                    transaction,
                    receipt: receipt.clone(),
                    rollback_only: false,
                });
                Ok(receipt)
            })
            .await
            .map_err(|_| HbError::Internal)?
    }

    /// Commands are serialized with commit/cancel and keep the current session
    /// for their entire lifetime, including Rules and relation/Schema reads.
    pub async fn execute<T: Send + 'static>(
        &self,
        transaction: Option<String>,
        persistent: bool,
        command: impl for<'a> FnOnce(DbSession<'a>) -> BoxFuture<'a, HbResult<T>> + Send + 'static,
    ) -> HbResult<T> {
        let inner = self.0.clone();
        self.0
            .executor
            .spawn(async move {
                let mut state = inner.state.lock().await;
                if inner.cancellation.is_cancelled() {
                    return Err(JsErrorKind::Timeout.into());
                }
                if let State::Unknown(receipt) = &*state {
                    return Err(unknown(receipt));
                }
                match (&mut *state, transaction) {
                    (State::Active(active), Some(id)) if active.receipt.id == id => {
                        let result = command(DbSession::Transaction(&active.transaction)).await;
                        if persistent && result.is_err() {
                            active.rollback_only = true;
                        }
                        result
                    }
                    (_, None) if !persistent => command(DbSession::Client(&inner.db)).await,
                    (State::Active(active), _) => {
                        if persistent {
                            active.rollback_only = true;
                        }
                        Err(HbError::Conflict(
                            "transaction identity does not match its owner".into(),
                        ))
                    }
                    _ => Err(HbError::Conflict(
                        "persistent commands require an active transaction".into(),
                    )),
                }
            })
            .await
            .map_err(|_| HbError::Internal)?
    }

    pub async fn mark_rollback_only(&self) {
        if let State::Active(active) = &mut *self.0.state.lock().await {
            active.rollback_only = true;
        }
    }

    pub async fn in_transaction(&self) -> bool {
        !matches!(*self.0.state.lock().await, State::Idle)
    }

    /// Serializes the transaction gate with begin/commit and the whole side effect.
    /// An already accepted send survives cancellation of its JS waiter.
    pub async fn side_effect<T: Send + 'static>(
        &self,
        operation: impl std::future::Future<Output = HbResult<T>> + Send + 'static,
    ) -> HbResult<T> {
        let inner = self.0.clone();
        self.0
            .executor
            .spawn(async move {
                let state = inner.state.lock().await;
                if !matches!(*state, State::Idle) {
                    return Err(JsErrorKind::SideEffect.into());
                }
                if inner.cancellation.is_cancelled() {
                    return Err(JsErrorKind::Timeout.into());
                }
                let result = operation.await;
                drop(state);
                result
            })
            .await
            .map_err(|_| HbError::Internal)?
    }

    pub async fn commit(&self, id: &str) -> HbResult<()> {
        let inner = self.0.clone();
        let id = id.to_owned();
        self.0.executor.spawn(async move {
            let mut state = inner.state.lock().await;
            match &*state {
                State::Unknown(receipt) => return Err(unknown(receipt)),
                State::Active(active) if active.receipt.id == id => {},
                _ => return Err(HbError::Conflict("transaction identity does not match its owner".into())),
            }
            let State::Active(active) = std::mem::replace(&mut *state, State::Idle) else { unreachable!() };
            if active.rollback_only || inner.cancellation.is_cancelled() {
                rollback(&inner.db, active, &mut state).await?;
                return Err(JsErrorKind::Aborted.into());
            }
            // Persist intent before handing commit to the database. Its marker
            // is written in the business transaction, never in the owner journal.
            if let Err(error) = set_status(&inner.db, &id, OperationState::Committing).await {
                rollback(&inner.db, active, &mut state).await?; return Err(error);
            }
            let marker = active.transaction.query("CREATE ONLY type::record('_operation_commits', $id) SET committed_at=time::now()")
                .bind(("id", id.clone())).await.map_err(database_error).and_then(|response| response.check().map_err(database_error));
            if let Err(error) = marker { rollback(&inner.db, active, &mut state).await?; return Err(error); }
            let receipt = active.receipt;
            let previous_receipt = inner.commit_receipt.lock().map_err(|_|HbError::Internal)?.replace(receipt.clone());
            // No cancellation select here: from this point the owner must learn
            // the outcome even if the caller, JS runtime, or HTTP connection left.
            match active.transaction.commit().await {
                Ok(_) => {
                    if let Err(error) = set_status(&inner.db, &id, OperationState::Committed).await {
                        tracing::error!(%id, %error, "commit confirmed; operation journal awaits reconciliation");
                    }
                    Ok(())
                }
                Err(error) => {
                    if matches!(error.details(), surrealdb::types::ErrorDetails::Query(Some(surrealdb::types::QueryError::TransactionConflict))) {
                        *inner.commit_receipt.lock().map_err(|_|HbError::Internal)? = previous_receipt;
                        // A typed conflict is an explicit database rollback, not
                        // an ambiguous transport outcome.
                        let _ = set_status(&inner.db, &id, OperationState::RolledBack).await;
                        return Err(HbError::Conflict("concurrent transaction changed the data".into()));
                    }
                    tracing::error!(%id, %error, "transaction commit requires reconciliation");
                    if matches!(has_commit_marker(&inner.db, &id).await, Ok(true)) {
                        let _ = set_status(&inner.db, &id, OperationState::Committed).await;
                        Ok(())
                    } else {
                        let _ = set_status(&inner.db, &id, OperationState::Unknown).await;
                        let error = unknown(&receipt); *state = State::Unknown(receipt); Err(error)
                    }
                }
            }
        }).await.map_err(|_| HbError::Internal)?
    }

    /// Finish is a barrier behind all accepted commands. An unresolved commit
    /// is retained for reconciliation; it is never compensated as a rollback.
    pub async fn finish(&self) -> HbResult<()> {
        let inner = self.0.clone();
        self.0
            .executor
            .spawn(async move {
                let mut state = inner.state.lock().await;
                match std::mem::replace(&mut *state, State::Idle) {
                    State::Active(active) => rollback(&inner.db, active, &mut state).await,
                    State::Unknown(receipt) => {
                        let error = unknown(&receipt);
                        *state = State::Unknown(receipt);
                        Err(error)
                    }
                    State::Idle => Ok(()),
                }
            })
            .await
            .map_err(|_| HbError::Internal)?
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let State::Active(active) = std::mem::replace(self.state.get_mut(), State::Idle) {
            let db = self.db.clone();
            self.executor.spawn(async move {
                let _ = rollback(&db, active, &mut State::Idle).await;
            });
        }
    }
}

async fn rollback(db: &DbClient, active: Active, state: &mut State) -> HbResult<()> {
    match active.transaction.cancel().await {
        Ok(_) => {
            // A journal write failure cannot change the confirmed rollback.
            if let Err(error) =
                set_status(db, &active.receipt.operation_id, OperationState::RolledBack).await
            {
                tracing::error!(%error, "rollback confirmed; operation journal awaits reconciliation");
            }
            Ok(())
        }
        Err(error) => {
            tracing::error!(%error, "transaction cancellation requires reconciliation");
            let _ = set_status(db, &active.receipt.operation_id, OperationState::Unknown).await;
            let error = unknown(&active.receipt);
            *state = State::Unknown(active.receipt);
            Err(error)
        }
    }
}

fn unknown(receipt: &TransactionReceipt) -> HbError {
    let mut error = JsError::new(JsErrorKind::CommitUnknown);
    error.details = Some(
        json!({"operationId":receipt.operation_id,"checkUrl":receipt.check_url,"checkCredential":receipt.check_credential}),
    );
    error.into()
}
fn credential_hash(credential: &str) -> String {
    format!("{:x}", Sha256::digest(credential.as_bytes()))
}

async fn set_status(db: &DbClient, id: &str, state: OperationState) -> HbResult<()> {
    db.inner()
        .query(
            "UPDATE ONLY type::record('_operations', $id) SET state=$state, updated_at=time::now()",
        )
        .bind(("id", id.to_owned()))
        .bind((
            "state",
            serde_json::to_value(state).map_err(database_error)?,
        ))
        .await
        .map_err(database_error)?
        .check()
        .map_err(database_error)?;
    Ok(())
}
async fn has_commit_marker(db: &DbClient, id: &str) -> HbResult<bool> {
    let mut response = db
        .inner()
        .query("SELECT id FROM type::record('_operation_commits', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(database_error)?
        .check()
        .map_err(database_error)?;
    let rows: Vec<Value> = response.take(0).map_err(database_error)?;
    Ok(!rows.is_empty())
}

/// Exposes status only, with no record data, token, principal, or credential hash.
pub async fn operation_status(
    db: &DbClient,
    id: &str,
    principal: Option<&str>,
    credential: Option<&str>,
) -> HbResult<OperationStatus> {
    let mut response = db
        .inner()
        .query("SELECT * FROM type::record('_operations', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(database_error)?
        .check()
        .map_err(database_error)?;
    let rows: Vec<Value> = response.take(0).map_err(database_error)?;
    let row = rows.first().ok_or(HbError::NotFound)?;
    let original = principal.is_some_and(|principal| row["principal"].as_str() == Some(principal));
    let credential = credential.is_some_and(|credential| {
        row["credentialHash"].as_str() == Some(credential_hash(credential).as_str())
    });
    if !original && !credential {
        return Err(HbError::NotFound);
    }
    let mut status: OperationStatus =
        serde_json::from_value(row.clone()).map_err(database_error)?;
    if matches!(
        status.state,
        OperationState::Committing | OperationState::Unknown
    ) && has_commit_marker(db, id).await?
    {
        status.state = OperationState::Committed;
    }
    Ok(status)
}

/// Call once before accepting requests, after opening the database. At restart
/// no old database transaction can still commit, so an absent marker is final.
pub async fn recover_operations(db: &DbClient) -> HbResult<()> {
    loop {
        let mut response = db.inner().query("SELECT operationId FROM _operations WHERE state IN ['pending','committing','unknown'] LIMIT 100")
            .await.map_err(database_error)?.check().map_err(database_error)?;
        let rows: Vec<Value> = response.take(0).map_err(database_error)?;
        if rows.is_empty() {
            return Ok(());
        }
        for row in rows {
            let id = row["operationId"].as_str().ok_or(HbError::Internal)?;
            let state = if has_commit_marker(db, id).await? {
                OperationState::Committed
            } else {
                OperationState::RolledBack
            };
            set_status(db, id, state).await?;
        }
    }
}
