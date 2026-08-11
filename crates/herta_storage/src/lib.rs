use std::{
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
    GetOptions, ObjectStore, ObjectStoreExt, PutPayload, aws::AmazonS3Builder,
    local::LocalFileSystem, memory::InMemory, path::Path, prefix::PrefixStore,
};
use tokio::io::AsyncReadExt;

const UPLOAD_PART_SIZE: usize = 8 * 1024 * 1024;

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

#[async_trait]
pub trait Storage: Send + Sync {
    async fn put_file(&self, key: &str, source: &FsPath) -> HbResult<ObjectMetadata>;
    async fn head(&self, key: &str) -> HbResult<ObjectMetadata>;
    async fn get(&self, key: &str, range: Option<Range<u64>>) -> HbResult<StoredObject>;
    async fn delete(&self, key: &str) -> HbResult<()>;
    async fn delete_prefix(&self, prefix: &str) -> HbResult<()>;
}

#[derive(Clone)]
pub struct ObjectStoreStorage {
    inner: Arc<dyn ObjectStore>,
}

impl ObjectStoreStorage {
    pub fn new(inner: Arc<dyn ObjectStore>) -> Self {
        Self { inner }
    }

    pub fn memory() -> Self {
        Self::new(Arc::new(InMemory::new()))
    }

    pub fn local(root: impl Into<PathBuf>) -> HbResult<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(storage_error)?;
        let store = LocalFileSystem::new_with_prefix(root).map_err(map_object_store_error)?;
        Ok(Self::new(Arc::new(store)))
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
    async fn put_file(&self, key: &str, source: &FsPath) -> HbResult<ObjectMetadata> {
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
        let location = storage_path(key)?;
        let metadata = self
            .inner
            .head(&location)
            .await
            .map_err(map_object_store_error)?;
        Ok(metadata.into())
    }

    async fn get(&self, key: &str, range: Option<Range<u64>>) -> HbResult<StoredObject> {
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
        let location = storage_path(key)?;
        match self.inner.delete(&location).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(error) => Err(map_object_store_error(error)),
        }
    }

    async fn delete_prefix(&self, prefix: &str) -> HbResult<()> {
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
