//! Transactional outbox state. Payloads are private to the native delivery worker.
use crate::{DbClient, DbSession};
use herta_core::{HbError, HbResult, JsErrorKind, outbox::*};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    #[serde(flatten)]
    pub receipt: OutboxReceipt,
    pub payload: Value,
    pub payload_hash: String,
    pub idempotency_key: String,
    pub lease_owner: Option<String>,
    pub lease_until: i64,
    pub resolved_by: Option<String>,
    pub resolution_note: Option<String>,
    /// Uncertainty survives subsequent attempts that are known not to have sent.
    #[serde(default)]
    pub uncertain: bool,
    #[serde(default)]
    pub preflight_failures: usize,
}
fn database(error: impl std::fmt::Display) -> HbError {
    HbError::Database(error.to_string())
}
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
pub async fn get(session: DbSession<'_>, id: &str) -> HbResult<Job> {
    let mut response = session
        .query("SELECT * FROM ONLY type::record('_extension_outbox', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(database)?
        .check()
        .map_err(database)?;
    let value: Option<Value> = response.take(0).map_err(database)?;
    serde_json::from_value(value.ok_or(HbError::NotFound)?).map_err(database)
}
pub async fn enqueue(
    session: DbSession<'_>,
    input: OutboxEnqueue,
    max_jobs: usize,
) -> HbResult<OutboxReceipt> {
    if input.idempotency_key.is_empty()
        || input.idempotency_key.len() > 128
        || !input
            .idempotency_key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
    {
        return Err(HbError::validation(
            "idempotencyKey must contain 1..128 ASCII letters, digits or -_.:",
        ));
    }
    let id = format!(
        "{:x}",
        Sha256::digest(format!(
            "{}\0{}",
            input.kind.as_str(),
            input.idempotency_key
        ))
    );
    let hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&input.payload).map_err(database)?)
    );
    match get(session, &id).await {
        Ok(job) if job.payload_hash == hash => return Ok(job.receipt),
        Ok(_) => return Err(JsErrorKind::IdempotencyConflict.into()),
        Err(HbError::NotFound) => {}
        Err(error) => return Err(error),
    }
    // Every enqueue touches one counter to prevent concurrent max_jobs write skew.
    let mut response = session
        .query("SELECT * FROM ONLY _extension_outbox_meta:quota")
        .await
        .map_err(database)?
        .check()
        .map_err(database)?;
    let counter: Option<Value> = response.take(0).map_err(database)?;
    let used = counter
        .as_ref()
        .and_then(|v| v["used"].as_u64())
        .unwrap_or(0);
    if used >= max_jobs as u64 {
        return Err(HbError::RateLimited);
    }
    let time = now();
    let job = Job {
        receipt: OutboxReceipt {
            job_id: id.clone(),
            kind: input.kind,
            state: OutboxState::Pending,
            attempts: 0,
            created_at: time,
            updated_at: time,
            next_attempt_at: time,
            error_code: None,
            result: None,
        },
        payload: input.payload,
        payload_hash: hash,
        idempotency_key: input.idempotency_key,
        lease_owner: None,
        lease_until: 0,
        resolved_by: None,
        resolution_note: None,
        uncertain: false,
        preflight_failures: 0,
    };
    session.query("UPSERT _extension_outbox_meta:quota SET used = $used; CREATE ONLY type::record('_extension_outbox', $id) CONTENT $job")
        .bind(("used",used+1)).bind(("id",id)).bind(("job",serde_json::to_value(&job).map_err(database)?))
        .await.map_err(database)?.check().map_err(database)?;
    Ok(job.receipt)
}

pub async fn list(db: &DbClient, after: &str, limit: usize) -> HbResult<Value> {
    if !(1..=500).contains(&limit) || after.len() > 64 {
        return Err(HbError::validation("invalid outbox page"));
    }
    let mut response = db
        .inner()
        .query("SELECT jobId, kind, state, attempts, createdAt, updatedAt, nextAttemptAt, errorCode, result FROM _extension_outbox WHERE jobId > $after ORDER BY jobId LIMIT $limit")
        .bind(("after", after.to_owned()))
        .bind(("limit", limit + 1))
        .await
        .map_err(database)?
        .check()
        .map_err(database)?;
    let rows: Vec<Value> = response.take(0).map_err(database)?;
    let more = rows.len() > limit;
    let jobs: Vec<OutboxReceipt> = rows
        .into_iter()
        .take(limit)
        .map(serde_json::from_value::<OutboxReceipt>)
        .collect::<Result<_, _>>()
        .map_err(database)?;
    let cursor = more.then(|| jobs.last().unwrap().job_id.clone());
    Ok(json!({"items":jobs,"nextCursor":cursor}))
}

/// Transactional CAS, including a unique attempt token; stale workers cannot acknowledge a new lease.
pub async fn claim(db: &DbClient, lease_seconds: u64) -> HbResult<Option<Job>> {
    let _guard = db.outbox_claim_lock().await;
    let transaction = db.inner().clone().begin().await.map_err(database)?;
    let time = now();
    let mut response=transaction.query("SELECT * FROM _extension_outbox WHERE state = 'pending' AND nextAttemptAt <= $now ORDER BY nextAttemptAt, jobId LIMIT 1")
        .bind(("now",time)).await.map_err(database)?.check().map_err(database)?;
    let rows: Vec<Value> = response.take(0).map_err(database)?;
    let Some(value) = rows.into_iter().next() else {
        transaction.cancel().await.map_err(database)?;
        return Ok(None);
    };
    let mut job: Job = serde_json::from_value(value).map_err(database)?;
    job.receipt.state = OutboxState::Leased;
    job.receipt.updated_at = time;
    job.lease_owner = Some(uuid::Uuid::now_v7().to_string());
    job.lease_until = time.saturating_add((lease_seconds as i64).saturating_mul(1000));
    save((&transaction).into(), &job).await?;
    transaction.commit().await.map_err(database)?;
    Ok(Some(job))
}
async fn save(session: DbSession<'_>, job: &Job) -> HbResult<()> {
    session
        .query("UPDATE type::record('_extension_outbox', $id) CONTENT $job")
        .bind(("id", job.receipt.job_id.clone()))
        .bind(("job", serde_json::to_value(job).map_err(database)?))
        .await
        .map_err(database)?
        .check()
        .map_err(database)?;
    Ok(())
}
pub async fn sending(db: &DbClient, job: &mut Job) -> HbResult<()> {
    job.receipt.state = OutboxState::Sending;
    job.receipt.attempts += 1;
    job.receipt.updated_at = now();
    update_owned(db, job).await
}
pub async fn update_owned(db: &DbClient, job: &Job) -> HbResult<()> {
    let mut response=db.inner().query("UPDATE type::record('_extension_outbox', $id) CONTENT $job WHERE leaseOwner = $owner AND state IN ['leased','sending'] RETURN jobId")
        .bind(("id",job.receipt.job_id.clone())).bind(("owner",job.lease_owner.clone()))
        .bind(("job",serde_json::to_value(job).map_err(database)?)).await.map_err(database)?.check().map_err(database)?;
    let rows: Vec<Value> = response.take(0).map_err(database)?;
    if rows.is_empty() {
        return Err(HbError::Conflict(
            "outbox lease no longer belongs to this worker".into(),
        ));
    }
    Ok(())
}
pub async fn renew(db: &DbClient, job: &Job, lease_seconds: u64) -> HbResult<()> {
    let mut response=db.inner().query("UPDATE type::record('_extension_outbox', $id) SET leaseUntil = $until WHERE leaseOwner = $owner AND state IN ['leased','sending'] RETURN jobId")
        .bind(("id",job.receipt.job_id.clone())).bind(("owner",job.lease_owner.clone()))
        .bind(("until",now().saturating_add((lease_seconds as i64).saturating_mul(1000))))
        .await.map_err(database)?.check().map_err(database)?;
    let rows: Vec<Value> = response.take(0).map_err(database)?;
    if rows.is_empty() {
        return Err(HbError::Conflict("outbox lease expired".into()));
    }
    Ok(())
}
pub async fn recover_expired(db: &DbClient) -> HbResult<()> {
    // A leased job has not entered the transport; sending may have. Unknown jobs
    // are never auto-cleared, even when a later configuration would allow retries.
    db.inner().query("UPDATE _extension_outbox SET state = 'pending', leaseOwner = NONE WHERE state = 'leased' AND leaseUntil <= $now RETURN NONE; UPDATE _extension_outbox SET state = 'unknown', uncertain = true, updatedAt = $now, errorCode = 'HB_OUTBOX_LEASE_EXPIRED' WHERE state = 'sending' AND leaseUntil <= $now RETURN NONE")
        .bind(("now",now())).await.map_err(database)?.check().map_err(database)?;
    Ok(())
}
pub async fn resolve(
    db: &DbClient,
    id: &str,
    input: OutboxResolve,
    administrator: String,
) -> HbResult<OutboxReceipt> {
    if input.note.trim().is_empty() || input.note.len() > 2000 {
        return Err(HbError::validation(
            "resolution requires a note of 1..2000 bytes",
        ));
    }
    let transaction = db.inner().clone().begin().await.map_err(database)?;
    let mut job = get((&transaction).into(), id).await?;
    if job.receipt.state != OutboxState::Unknown {
        return Err(HbError::Conflict(
            "only unknown jobs can be resolved".into(),
        ));
    }
    job.receipt.state = match input.resolution {
        OutboxResolution::Accepted => OutboxState::Accepted,
        OutboxResolution::NotSent => OutboxState::Pending,
    };
    job.receipt.updated_at = now();
    job.receipt.next_attempt_at = now();
    job.receipt.error_code = None;
    job.lease_owner = None;
    job.lease_until = 0;
    job.resolved_by = Some(administrator);
    job.resolution_note = Some(input.note);
    job.uncertain = false;
    job.preflight_failures = 0;
    save((&transaction).into(), &job).await?;
    transaction.commit().await.map_err(database)?;
    Ok(job.receipt)
}
pub async fn purge(db: &DbClient, retention_days: u64) -> HbResult<()> {
    let cutoff = now().saturating_sub((retention_days as i64).saturating_mul(86_400_000));
    let transaction = db.inner().clone().begin().await.map_err(database)?;
    let mut result=transaction.query("SELECT VALUE jobId FROM _extension_outbox WHERE state IN ['accepted','failed'] AND updatedAt < $cutoff LIMIT 100")
        .bind(("cutoff",cutoff)).await.map_err(database)?.check().map_err(database)?;
    let rows: Vec<String> = result.take(0).map_err(database)?;
    if !rows.is_empty() {
        transaction
            .query("DELETE _extension_outbox WHERE jobId IN $ids; UPDATE _extension_outbox_meta:quota SET used -= $count")
            .bind(("count", rows.len()))
            .bind(("ids", rows))
            .await
            .map_err(database)?
            .check()
            .map_err(database)?;
    }
    transaction.commit().await.map_err(database)?;
    Ok(())
}
