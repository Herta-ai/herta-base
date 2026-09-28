//! Root-scoped accounting for bytes crossing the worker/host boundary.
use crate::{HbError, HbResult};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Debug)]
pub struct HostBudget(Arc<Inner>);
#[derive(Debug)]
struct Inner {
    limit: usize,
    used: AtomicUsize,
}

#[derive(Debug)]
pub struct BufferPermit {
    budget: HostBudget,
    size: usize,
}
impl Drop for BufferPermit {
    fn drop(&mut self) {
        self.budget.0.used.fetch_sub(self.size, Ordering::AcqRel);
    }
}
impl HostBudget {
    /// Accept an already bounded transport buffer without copying it.
    pub fn adopt(&self, bytes: Vec<u8>) -> HbResult<HostBuffer> {
        let permit = self.reserve(bytes.capacity())?;
        Ok(HostBuffer {
            bytes,
            _permit: permit,
        })
    }
    pub fn new(limit: usize) -> Self {
        Self(Arc::new(Inner {
            limit,
            used: AtomicUsize::new(0),
        }))
    }
    pub fn reserve(&self, size: usize) -> HbResult<BufferPermit> {
        self.0
            .used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(size).filter(|next| *next <= self.0.limit)
            })
            .map_err(|_| HbError::PayloadTooLarge)?;
        Ok(BufferPermit {
            budget: self.clone(),
            size,
        })
    }
    pub fn allocate(&self, size: usize) -> HbResult<HostBuffer> {
        let permit = self.reserve(size)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|_| HbError::PayloadTooLarge)?;
        bytes.resize(size, 0);
        Ok(HostBuffer {
            bytes,
            _permit: permit,
        })
    }
    pub fn copy(&self, bytes: &[u8]) -> HbResult<HostBuffer> {
        let mut buffer = self.allocate(bytes.len())?;
        buffer.as_mut().copy_from_slice(bytes);
        Ok(buffer)
    }
}

/// The reservation follows the allocation, including native operations whose JS
/// waiter was cancelled. Storage can retain this as the owner of a bytes::Bytes.
#[derive(Debug)]
pub struct HostBuffer {
    bytes: Vec<u8>,
    _permit: BufferPermit,
}
impl AsRef<[u8]> for HostBuffer {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}
impl AsMut<[u8]> for HostBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

#[derive(Debug)]
pub struct HostReply {
    pub value: serde_json::Value,
    pub bytes: Option<HostBuffer>,
}
impl From<serde_json::Value> for HostReply {
    fn from(value: serde_json::Value) -> Self {
        Self { value, bytes: None }
    }
}
impl HostReply {
    /// Compatibility path for callers of the original JSON-only dispatcher.
    pub fn into_response_value(mut self) -> HbResult<serde_json::Value> {
        if let Some(bytes) = self.bytes {
            let value = self.value.as_object_mut().ok_or(HbError::Internal)?;
            value.insert(
                "body".into(),
                serde_json::to_value(bytes.as_ref()).map_err(|_| HbError::Internal)?,
            );
        }
        Ok(self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_budget_releases_only_when_allocation_is_dropped() {
        let budget = HostBudget::new(8);
        let bytes = budget.copy(b"12345").unwrap();
        assert!(budget.clone().allocate(4).is_err());
        let three = budget.reserve(3).unwrap();
        assert!(budget.reserve(usize::MAX).is_err());
        drop(bytes);
        assert!(budget.allocate(5).is_ok());
        drop(three);
        assert!(budget.allocate(8).is_ok());
    }
}
