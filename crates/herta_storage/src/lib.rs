use std::{
    collections::BTreeMap,
    io,
    ops::Range,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures_util::{StreamExt, TryStreamExt, stream::BoxStream};
use herta_core::{HbConfig, HbError, HbResult};
use object_store::{
    GetOptions, ObjectStore, ObjectStoreExt, PutPayload, aws::AmazonS3Builder, memory::InMemory,
    path::Path, prefix::PrefixStore,
};
use tokio::io::AsyncReadExt;

const UPLOAD_PART_SIZE: usize = 8 * 1024 * 1024;
mod local;

pub type StorageStream = BoxStream<'static, Result<Bytes, io::Error>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMetadata {
    pub size: u64,
    pub e_tag: Option<String>,
    pub last_modified: DateTime<Utc>,
}

pub struct StoredObject {
    pub metadata: ObjectMetadata,
    pub range: Range<u64>,
    pub stream: StorageStream,
}

#[derive(Debug)]
pub struct ObjectPage {
    pub items: Vec<(String, ObjectMetadata)>,
    pub next_cursor: Option<String>,
}

#[async_trait]
pub trait Storage: Send + Sync {
    async fn put_bytes(&self, _key: &str, _bytes: Bytes) -> HbResult<ObjectMetadata> {
        Err(HbError::CapabilityUnavailable)
    }
    /// Bounded listing for recovery scans. Rejects a prefix with more than limit entries.
    async fn list(&self, _prefix: &str, _limit: usize) -> HbResult<Vec<(String, ObjectMetadata)>> {
        Err(HbError::CapabilityUnavailable)
    }
    /// Sorted keyset page of objects below a directory prefix. Cursors are bound to
    /// that prefix. Concurrent writes do not provide a cross-page snapshot.
    async fn list_page(
        &self,
        _prefix: &str,
        _limit: usize,
        _cursor: Option<&str>,
    ) -> HbResult<ObjectPage> {
        Err(HbError::CapabilityUnavailable)
    }
    async fn copy(
        &self,
        _source: &str,
        _destination: &str,
        _max_bytes: u64,
    ) -> HbResult<ObjectMetadata> {
        Err(HbError::CapabilityUnavailable)
    }
    async fn put_file(&self, key: &str, source: &FsPath) -> HbResult<ObjectMetadata>;
    async fn head(&self, key: &str) -> HbResult<ObjectMetadata>;
    async fn get(&self, key: &str, range: Option<Range<u64>>) -> HbResult<StoredObject>;
    async fn delete(&self, key: &str) -> HbResult<()>;
    async fn delete_prefix(&self, prefix: &str) -> HbResult<()>;
}

#[derive(Clone)]
pub struct ObjectStoreStorage {
    inner: Arc<dyn ObjectStore>,
    local: Option<Arc<local::LocalStorage>>,
}

impl ObjectStoreStorage {
    pub fn new(inner: Arc<dyn ObjectStore>) -> Self {
        Self { inner, local: None }
    }

    pub fn memory() -> Self {
        Self::new(Arc::new(InMemory::new()))
    }

    pub fn local(root: impl Into<PathBuf>) -> HbResult<Self> {
        let root = root.into();
        let local = local::LocalStorage::new(&root)?;
        Ok(Self {
            inner: Arc::new(InMemory::new()),
            local: Some(Arc::new(local)),
        })
    }

    pub fn s3(config: &herta_core::S3Config) -> HbResult<Self> {
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(&config.bucket)
            .with_region(&config.region)
            .with_virtual_hosted_style_request(!config.force_path_style)
            .with_allow_http(config.allow_http);
        if let Some(endpoint) = &config.endpoint {
            builder = builder.with_endpoint(endpoint);
        }
        if let Some(access_key) = &config.access_key {
            builder = builder.with_access_key_id(access_key);
        }
        if let Some(secret_key) = &config.secret_key {
            builder = builder.with_secret_access_key(secret_key);
        }
        if let Some(session_token) = &config.session_token {
            builder = builder.with_token(session_token);
        }
        let store = builder.build().map_err(storage_error)?;
        let inner: Arc<dyn ObjectStore> = if config.prefix.trim_matches('/').is_empty() {
            Arc::new(store)
        } else {
            let prefix = storage_path(config.prefix.trim_matches('/'))?;
            Arc::new(PrefixStore::new(store, prefix))
        };
        Ok(Self::new(inner))
    }
}

pub fn storage_from_config(config: &HbConfig) -> HbResult<Arc<dyn Storage>> {
    let storage = if config.storage.storage_type == "s3" {
        ObjectStoreStorage::s3(&config.storage.s3)?
    } else {
        ObjectStoreStorage::local(PathBuf::from(&config.paths.data_dir).join("storage"))?
    };
    Ok(Arc::new(storage))
}

#[async_trait]
impl Storage for ObjectStoreStorage {
    async fn put_bytes(&self, key: &str, bytes: Bytes) -> HbResult<ObjectMetadata> {
        if let Some(local) = &self.local {
            return local.put_bytes(key, bytes).await;
        }
        self.inner
            .put(&storage_path(key)?, PutPayload::from(bytes))
            .await
            .map_err(map_object_store_error)?;
        self.head(key).await
    }
    async fn list(&self, prefix: &str, limit: usize) -> HbResult<Vec<(String, ObjectMetadata)>> {
        let page = self.list_page(prefix, limit, None).await?;
        if page.next_cursor.is_some() {
            return Err(HbError::PayloadTooLarge);
        }
        Ok(page.items)
    }
    async fn list_page(
        &self,
        prefix: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> HbResult<ObjectPage> {
        let mut page = PageBuilder::new(prefix, limit, cursor)?;
        if let Some(local) = &self.local {
            return local.list_page(page).await;
        }
        let prefix = storage_path(&page.prefix)?;
        let offset = (!page.after.is_empty())
            .then(|| storage_path(&page.after))
            .transpose()?;
        let mut objects = match &offset {
            Some(offset) => self.inner.list_with_offset(Some(&prefix), offset),
            None => self.inner.list(Some(&prefix)),
        };
        // ObjectStore deliberately makes no ordering guarantee. Retain the smallest
        // limit + 1 keys while streaming, including across S3 provider pages.
        while let Some(object) = objects.try_next().await.map_err(map_object_store_error)? {
            let key = object.location.to_string();
            if page.accepts(&key) {
                page.insert(key, object.into());
            }
        }
        page.finish()
    }
    async fn copy(
        &self,
        source: &str,
        destination: &str,
        max_bytes: u64,
    ) -> HbResult<ObjectMetadata> {
        if let Some(local) = &self.local {
            return local.copy(source, destination, max_bytes).await;
        }
        if self.head(source).await?.size > max_bytes {
            return Err(HbError::PayloadTooLarge);
        }
        self.inner
            .copy(&storage_path(source)?, &storage_path(destination)?)
            .await
            .map_err(map_object_store_error)?;
        let metadata = self.head(destination).await?;
        if metadata.size > max_bytes {
            return Err(HbError::PayloadTooLarge);
        }
        Ok(metadata)
    }
    async fn put_file(&self, key: &str, source: &FsPath) -> HbResult<ObjectMetadata> {
        if let Some(local) = &self.local {
            return local.put_file(key, source).await;
        }
        let location = storage_path(key)?;
        let mut file = tokio::fs::File::open(source).await.map_err(storage_error)?;
        if file.metadata().await.map_err(storage_error)?.len() == 0 {
            self.inner
                .put(&location, PutPayload::from(Bytes::new()))
                .await
                .map_err(map_object_store_error)?;
            return self.head(key).await;
        }
        let mut upload = self
            .inner
            .put_multipart(&location)
            .await
            .map_err(map_object_store_error)?;

        loop {
            let mut buffer = vec![0_u8; UPLOAD_PART_SIZE];
            let count = match file.read(&mut buffer).await {
                Ok(count) => count,
                Err(error) => {
                    let _ = upload.abort().await;
                    return Err(storage_error(error));
                }
            };
            if count == 0 {
                break;
            }
            buffer.truncate(count);
            if let Err(error) = upload.put_part(PutPayload::from(buffer)).await {
                let _ = upload.abort().await;
                return Err(map_object_store_error(error));
            }
        }

        upload.complete().await.map_err(map_object_store_error)?;
        self.head(key).await
    }

    async fn head(&self, key: &str) -> HbResult<ObjectMetadata> {
        if let Some(local) = &self.local {
            return local.head(key).await;
        }
        let location = storage_path(key)?;
        let metadata = self
            .inner
            .head(&location)
            .await
            .map_err(map_object_store_error)?;
        Ok(metadata.into())
    }

    async fn get(&self, key: &str, range: Option<Range<u64>>) -> HbResult<StoredObject> {
        if let Some(local) = &self.local {
            return local.get(key, range).await;
        }
        let location = storage_path(key)?;
        let result = self
            .inner
            .get_opts(&location, GetOptions::new().with_range(range))
            .await
            .map_err(map_object_store_error)?;
        let metadata = result.meta.clone().into();
        let range = result.range.clone();
        let stream = result
            .into_stream()
            .map(|item| item.map_err(io::Error::from))
            .boxed();
        Ok(StoredObject {
            metadata,
            range,
            stream,
        })
    }

    async fn delete(&self, key: &str) -> HbResult<()> {
        if let Some(local) = &self.local {
            return local.delete(key).await;
        }
        let location = storage_path(key)?;
        match self.inner.delete(&location).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(error) => Err(map_object_store_error(error)),
        }
    }

    async fn delete_prefix(&self, prefix: &str) -> HbResult<()> {
        if let Some(local) = &self.local {
            return local.delete_prefix(prefix).await;
        }
        let prefix = storage_path(prefix.trim_end_matches('/'))?;
        let locations = self
            .inner
            .list(Some(&prefix))
            .map_ok(|metadata| metadata.location)
            .boxed();
        let mut deleted = self.inner.delete_stream(locations);
        while let Some(result) = deleted.next().await {
            result.map_err(map_object_store_error)?;
        }
        Ok(())
    }
}

struct PageBuilder {
    prefix: String,
    after: String,
    limit: usize,
    items: BTreeMap<String, ObjectMetadata>,
}
impl PageBuilder {
    fn new(prefix: &str, limit: usize, cursor: Option<&str>) -> HbResult<Self> {
        if !(1..=10000).contains(&limit) {
            return Err(HbError::validation("storage list limit must be 1..10000"));
        }
        let prefix = prefix.trim_end_matches('/').to_owned();
        validate_key(&prefix)?;
        let after = if let Some(cursor) = cursor {
            let (scope, after): (String, String) = serde_json::from_str(cursor)
                .map_err(|_| HbError::validation("invalid storage cursor"))?;
            validate_key(&after)?;
            if scope != prefix || !after.starts_with(&format!("{prefix}/")) {
                return Err(HbError::validation(
                    "storage cursor belongs to another prefix",
                ));
            }
            after
        } else {
            String::new()
        };
        Ok(Self {
            prefix,
            after,
            limit,
            items: BTreeMap::new(),
        })
    }
    fn accepts(&self, key: &str) -> bool {
        key.starts_with(&self.prefix)
            && key.as_bytes().get(self.prefix.len()) == Some(&b'/')
            && key > self.after.as_str()
            && (self.items.len() <= self.limit
                || self
                    .items
                    .last_key_value()
                    .is_some_and(|(last, _)| key < last.as_str()))
    }
    fn insert(&mut self, key: String, metadata: ObjectMetadata) {
        self.items.insert(key, metadata);
        if self.items.len() > self.limit + 1 {
            self.items.pop_last();
        }
    }
    fn finish(mut self) -> HbResult<ObjectPage> {
        let more = self.items.len() > self.limit;
        if more {
            self.items.pop_last();
        }
        let next_cursor = if more {
            Some(
                serde_json::to_string(&(&self.prefix, self.items.last_key_value().unwrap().0))
                    .map_err(|_| HbError::Internal)?,
            )
        } else {
            None
        };
        Ok(ObjectPage {
            items: self.items.into_iter().collect(),
            next_cursor,
        })
    }
}

impl From<object_store::ObjectMeta> for ObjectMetadata {
    fn from(value: object_store::ObjectMeta) -> Self {
        Self {
            size: value.size,
            e_tag: value.e_tag,
            last_modified: value.last_modified,
        }
    }
}

pub fn validate_key(key: &str) -> HbResult<()> {
    if key.is_empty()
        || key.starts_with('/')
        || key.ends_with('/')
        || key.contains('\\')
        || key.contains('\0')
        || key
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(HbError::validation("invalid storage key"));
    }
    Ok(())
}

fn storage_path(key: &str) -> HbResult<Path> {
    validate_key(key)?;
    Path::parse(key).map_err(|error| HbError::validation(format!("invalid storage key: {error}")))
}

fn map_object_store_error(error: object_store::Error) -> HbError {
    match error {
        object_store::Error::NotFound { .. } => HbError::NotFound,
        error => HbError::Storage(error.to_string()),
    }
}

fn storage_error(error: impl std::fmt::Display) -> HbError {
    HbError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_keys() {
        for key in ["", "/absolute", "../escape", "a/../b", "a\\b", "a//b"] {
            assert!(validate_key(key).is_err(), "key should be rejected: {key}");
        }
        assert!(validate_key("records/posts/id/avatar/file.png").is_ok());
    }

    #[tokio::test]
    async fn local_storage_supports_lifecycle_and_ranges() {
        let root = tempfile::tempdir().unwrap();
        let source_dir = tempfile::tempdir().unwrap();
        let source = source_dir.path().join("payload.bin");
        tokio::fs::write(&source, b"0123456789").await.unwrap();
        let storage = ObjectStoreStorage::local(root.path()).unwrap();
        let key = "records/posts/id/file/payload.bin";

        let metadata = storage.put_file(key, &source).await.unwrap();
        assert_eq!(metadata.size, 10);
        let object = storage.get(key, Some(2..6)).await.unwrap();
        assert_eq!(object.range, 2..6);
        let content = object.stream.try_collect::<Vec<_>>().await.unwrap();
        assert_eq!(content.concat(), b"2345");

        storage.delete(key).await.unwrap();
        assert!(matches!(storage.head(key).await, Err(HbError::NotFound)));
    }

    #[tokio::test]
    async fn bounded_listing_copy_and_atomic_overwrite_match_both_backends() {
        let root = tempfile::tempdir().unwrap();
        for storage in [
            ObjectStoreStorage::memory(),
            ObjectStoreStorage::local(root.path()).unwrap(),
        ] {
            storage
                .put_bytes("extensions/a", Bytes::from_static(b"first"))
                .await
                .unwrap();
            storage
                .copy("extensions/a", "extensions/b", 5)
                .await
                .unwrap();
            assert!(matches!(
                storage.copy("extensions/a", "extensions/c", 4).await,
                Err(HbError::PayloadTooLarge)
            ));
            assert!(matches!(
                storage.head("extensions/c").await,
                Err(HbError::NotFound)
            ));
            assert!(matches!(
                storage.list("extensions", 1).await,
                Err(HbError::PayloadTooLarge)
            ));
            let items = storage.list("extensions", 2).await.unwrap();
            assert_eq!(
                items.iter().map(|item| item.0.as_str()).collect::<Vec<_>>(),
                ["extensions/a", "extensions/b"]
            );
            let (first, second) = tokio::join!(
                storage.put_bytes("extensions/a", Bytes::from_static(b"one")),
                storage.put_bytes("extensions/a", Bytes::from_static(b"two"))
            );
            first.unwrap();
            second.unwrap();
            let bytes = storage
                .get("extensions/a", None)
                .await
                .unwrap()
                .stream
                .try_collect::<Vec<_>>()
                .await
                .unwrap()
                .concat();
            assert!(bytes == b"one" || bytes == b"two");
            storage.delete_prefix("extensions").await.unwrap();
            assert!(storage.list("extensions", 2).await.unwrap().is_empty());
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn local_storage_rejects_windows_junctions_without_touching_the_target() {
        use std::os::windows::process::CommandExt;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("sentinel"), b"unchanged").unwrap();
        let junction = root.path().join("escape");
        let result = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(outside.path())
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let storage = ObjectStoreStorage::local(root.path()).unwrap();
        assert!(storage.get("escape/sentinel", None).await.is_err());
        assert!(
            storage
                .put_bytes("escape/sentinel", Bytes::from_static(b"changed"))
                .await
                .is_err()
        );
        assert!(storage.delete("escape/sentinel").await.is_err());
        assert!(storage.delete_prefix("escape").await.is_err());
        assert!(storage.list("escape", 10).await.is_err());
        assert!(storage.list_page("escape", 10, None).await.is_err());
        assert!(ObjectStoreStorage::local(&junction).is_err());
        assert_eq!(
            std::fs::read(outside.path().join("sentinel")).unwrap(),
            b"unchanged"
        );
        // Remove the junction itself, never recurse through it.
        std::fs::remove_dir(junction).unwrap();
    }

    #[tokio::test]
    async fn physical_pages_are_sorted_scoped_and_survive_deleted_cursor_objects() {
        let root = tempfile::tempdir().unwrap();
        for storage in [
            ObjectStoreStorage::memory(),
            ObjectStoreStorage::local(root.path()).unwrap(),
        ] {
            let keys = [
                "scan/z",
                "scan/a/x",
                "scan/a-b",
                "scan/a.b",
                "scan/b",
                "scan/深/文",
                "scan/深-文",
            ];
            for key in keys {
                storage
                    .put_bytes(key, Bytes::from_static(b"x"))
                    .await
                    .unwrap();
            }
            storage
                .put_bytes("scan-other/keep", Bytes::new())
                .await
                .unwrap();
            let mut cursor = None;
            let mut found = Vec::new();
            loop {
                let page = storage
                    .list_page("scan/", 2, cursor.as_deref())
                    .await
                    .unwrap();
                assert!(page.items.len() <= 2);
                for (key, metadata) in page.items {
                    assert_eq!(metadata.size, 1);
                    storage.delete(&key).await.unwrap();
                    found.push(key);
                }
                cursor = page.next_cursor;
                if cursor.is_none() {
                    break;
                }
                assert!(
                    storage
                        .list_page("scan-other", 2, cursor.as_deref())
                        .await
                        .is_err()
                );
            }
            let mut expected = keys.map(str::to_owned).to_vec();
            expected.sort();
            assert_eq!(found, expected);
            assert!(storage.head("scan-other/keep").await.is_ok());
            assert!(storage.list_page("scan", 0, None).await.is_err());
            assert!(storage.list_page("scan", 10001, None).await.is_err());
            assert!(storage.list_page("scan", 1, Some("invalid")).await.is_err());
        }
    }

    #[tokio::test]
    async fn local_cleanup_handles_more_than_ten_thousand_entries_in_one_directory() {
        let root = tempfile::tempdir().unwrap();
        let objects = root.path().join("large");
        std::fs::create_dir(&objects).unwrap();
        for index in (0..10003).rev() {
            std::fs::write(objects.join(format!("{index:05}")), []).unwrap();
        }
        let storage = ObjectStoreStorage::local(root.path()).unwrap();
        let mut cursor = None;
        let mut count = 0;
        loop {
            let page = storage
                .list_page("large", 1000, cursor.as_deref())
                .await
                .unwrap();
            for (key, _) in &page.items {
                assert_eq!(key, &format!("large/{count:05}"));
                count += 1;
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(count, 10003);
        storage.delete_prefix("large").await.unwrap();
        assert!(storage.list("large", 1).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_prefix_removes_only_matching_objects() {
        let source_dir = tempfile::tempdir().unwrap();
        let source = source_dir.path().join("payload.bin");
        tokio::fs::write(&source, b"data").await.unwrap();
        let storage = ObjectStoreStorage::memory();
        storage
            .put_file("records/a/1/f/a.bin", &source)
            .await
            .unwrap();
        storage
            .put_file("records/b/1/f/b.bin", &source)
            .await
            .unwrap();

        storage.delete_prefix("records/a").await.unwrap();
        assert!(matches!(
            storage.head("records/a/1/f/a.bin").await,
            Err(HbError::NotFound)
        ));
        assert!(storage.head("records/b/1/f/b.bin").await.is_ok());
    }

    #[tokio::test]
    async fn empty_and_failed_uploads_do_not_leave_partial_objects() {
        let root = tempfile::tempdir().unwrap();
        let source_dir = tempfile::tempdir().unwrap();
        let empty = source_dir.path().join("empty.bin");
        tokio::fs::write(&empty, []).await.unwrap();
        let storage = ObjectStoreStorage::local(root.path()).unwrap();

        let metadata = storage
            .put_file("records/a/1/f/empty.bin", &empty)
            .await
            .unwrap();
        assert_eq!(metadata.size, 0);
        assert!(
            storage
                .put_file(
                    "records/a/1/f/missing.bin",
                    &source_dir.path().join("missing.bin")
                )
                .await
                .is_err()
        );
        assert!(matches!(
            storage.head("records/a/1/f/missing.bin").await,
            Err(HbError::NotFound)
        ));
    }

    #[test]
    fn s3_builder_accepts_private_compatible_endpoint_configuration() {
        let config = herta_core::S3Config {
            endpoint: Some("https://s3.example.test".into()),
            bucket: "files".into(),
            region: "us-east-1".into(),
            prefix: "tenant".into(),
            force_path_style: true,
            allow_http: false,
            access_key: Some("access".into()),
            secret_key: Some("secret".into()),
            session_token: None,
        };
        ObjectStoreStorage::s3(&config).unwrap();
    }
}
