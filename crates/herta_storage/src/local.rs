//! Local objects are always accessed through no-follow directory and file handles.
use super::*;
use herta_core::sandbox_fs::SandboxDir;
use std::io::{Read, Seek, SeekFrom, Write};

pub(super) struct LocalStorage {
    root: Arc<SandboxDir>,
}
impl LocalStorage {
    pub fn new(root: &FsPath) -> HbResult<Self> {
        Ok(Self {
            root: Arc::new(SandboxDir::open(root, true).map_err(local_error)?),
        })
    }
    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&SandboxDir) -> HbResult<T> + Send + 'static,
    ) -> HbResult<T> {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || operation(&root))
            .await
            .map_err(|_| HbError::Internal)?
    }
    pub async fn put_file(&self, key: &str, source: &FsPath) -> HbResult<ObjectMetadata> {
        let key = key.to_owned();
        let source = source.to_owned();
        self.blocking(move |root| {
            let source = std::fs::File::open(source).map_err(local_error)?;
            write(root, &key, source, u64::MAX)
        })
        .await
    }
    pub async fn put_bytes(&self, key: &str, bytes: Bytes) -> HbResult<ObjectMetadata> {
        let key = key.to_owned();
        self.blocking(move |root| write(root, &key, std::io::Cursor::new(bytes), u64::MAX))
            .await
    }
    pub async fn head(&self, key: &str) -> HbResult<ObjectMetadata> {
        let key = key.to_owned();
        self.blocking(move |root| {
            let (parent, name) = parent(root, &key, false)?;
            metadata(&parent.read(&name).map_err(local_error)?.into_std())
        })
        .await
    }
    pub async fn get(&self, key: &str, range: Option<Range<u64>>) -> HbResult<StoredObject> {
        let key = key.to_owned();
        let (file, metadata, range) = self
            .blocking(move |root| {
                let (parent, name) = parent(root, &key, false)?;
                let mut file = parent.read(&name).map_err(local_error)?.into_std();
                let metadata = metadata(&file)?;
                let range = range.unwrap_or(0..metadata.size);
                if range.end > metadata.size || range.start > range.end {
                    return Err(HbError::RangeNotSatisfiable);
                }
                file.seek(SeekFrom::Start(range.start))
                    .map_err(local_error)?;
                Ok((file, metadata, range))
            })
            .await?;
        let file = tokio::fs::File::from_std(file).take(range.end - range.start);
        Ok(StoredObject {
            metadata,
            range,
            stream: tokio_util::io::ReaderStream::new(file).boxed(),
        })
    }
    pub async fn copy(
        &self,
        source: &str,
        destination: &str,
        limit: u64,
    ) -> HbResult<ObjectMetadata> {
        let source = source.to_owned();
        let destination = destination.to_owned();
        self.blocking(move |root| {
            let (parent, name) = parent(root, &source, false)?;
            let source = parent.read(&name).map_err(local_error)?.into_std();
            if source.metadata().map_err(local_error)?.len() > limit {
                return Err(HbError::PayloadTooLarge);
            }
            write(root, &destination, source, limit)
        })
        .await
    }
    pub async fn delete(&self, key: &str) -> HbResult<()> {
        let key = key.to_owned();
        self.blocking(move |root| {
            match parent(root, &key, false).and_then(|(parent, name)| {
                // Opening first rejects special files and links. Unlink itself never follows a replaced link.
                drop(parent.read(&name).map_err(local_error)?);
                parent.remove(&name).map_err(local_error)
            }) {
                Err(HbError::NotFound) => Ok(()),
                result => result,
            }
        })
        .await
    }
    pub async fn list_page(&self, mut page: PageBuilder) -> HbResult<ObjectPage> {
        self.blocking(move |root| {
            let mut directory = root.try_clone().map_err(local_error)?;
            let prefix = page.prefix.clone();
            for name in prefix.split('/') {
                directory = match directory.child(name, false).map_err(local_error) {
                    Ok(dir) => dir,
                    Err(HbError::NotFound) => return page.finish(),
                    Err(error) => return Err(error),
                };
            }
            walk(&directory, &prefix, 0, &mut page)?;
            page.finish()
        })
        .await
    }
    pub async fn delete_prefix(&self, prefix: &str) -> HbResult<()> {
        // This management operation deliberately enumerates bounded batches. Empty
        // directories stay in place; it never recursively deletes a path by name.
        let prefix = prefix.trim_end_matches('/').to_owned();
        self.blocking(move |root| {
            validate_key(&prefix)?;
            let mut directory = root.try_clone().map_err(local_error)?;
            for name in prefix.split('/') {
                directory = match directory.child(name, false).map_err(local_error) {
                    Ok(dir) => dir,
                    Err(HbError::NotFound) => return Ok(()),
                    Err(error) => return Err(error),
                };
            }
            remove_children(&directory, 0)
        })
        .await
    }
}
fn parent(root: &SandboxDir, key: &str, create: bool) -> HbResult<(SandboxDir, String)> {
    validate_key(key)?;
    let mut parts = key.split('/').peekable();
    let mut directory = root.try_clone().map_err(local_error)?;
    while let Some(name) = parts.next() {
        herta_core::sandbox_fs::validate_component(name).map_err(local_error)?;
        if parts.peek().is_none() {
            return Ok((directory, name.into()));
        }
        directory = directory.child(name, create).map_err(local_error)?;
    }
    Err(HbError::validation("empty storage key"))
}
fn write(
    root: &SandboxDir,
    key: &str,
    mut source: impl Read,
    limit: u64,
) -> HbResult<ObjectMetadata> {
    let (directory, name) = parent(root, key, true)?;
    let temporary = format!(".hb-{}.tmp", uuid::Uuid::now_v7());
    let result = (|| {
        let mut target = directory
            .create_new(&temporary)
            .map_err(local_error)?
            .into_std();
        let mut bytes = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let size = source.read(&mut buffer).map_err(local_error)?;
            if size == 0 {
                break;
            }
            bytes = bytes
                .checked_add(size as u64)
                .ok_or(HbError::PayloadTooLarge)?;
            if bytes > limit {
                return Err(HbError::PayloadTooLarge);
            }
            target.write_all(&buffer[..size]).map_err(local_error)?;
        }
        target.sync_all().map_err(local_error)?;
        let metadata = metadata(&target)?;
        drop(target);
        // Replacing a symlink by rename cannot follow it, but reject a known link
        // to keep all local operations under the same no-links contract.
        match directory.read(&name).map_err(local_error) {
            Ok(file) => drop(file),
            Err(HbError::NotFound) => {}
            Err(error) => return Err(error),
        }
        directory
            .rename(&temporary, &directory, &name)
            .map_err(local_error)?;
        Ok(metadata)
    })();
    if result.is_err() {
        let _ = directory.remove(&temporary);
    }
    result
}
fn metadata(file: &std::fs::File) -> HbResult<ObjectMetadata> {
    let info = file.metadata().map_err(local_error)?;
    let time = info.modified().map_err(local_error)?;
    let stamp = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(ObjectMetadata {
        size: info.len(),
        last_modified: time.into(),
        e_tag: Some(format!("{stamp:x}-{:x}", info.len())),
    })
}
fn local_error(error: io::Error) -> HbError {
    if error.kind() == io::ErrorKind::NotFound {
        HbError::NotFound
    } else {
        storage_error(error)
    }
}
fn walk(
    directory: &SandboxDir,
    prefix: &str,
    depth: usize,
    page: &mut PageBuilder,
) -> HbResult<()> {
    if depth > 32 {
        return Err(HbError::PayloadTooLarge);
    }
    for entry in directory.iter().map_err(local_error)? {
        let (name, is_dir, is_file) = entry.map_err(local_error)?;
        let path = format!("{prefix}/{name}");
        if is_dir {
            walk(
                &directory.child(&name, false).map_err(local_error)?,
                &path,
                depth + 1,
                page,
            )?;
        } else if is_file {
            if page.accepts(&path) {
                page.insert(
                    path,
                    metadata(&directory.read(&name).map_err(local_error)?.into_std())?,
                );
            }
        } else {
            return Err(storage_error("non-regular entry in storage"));
        }
    }
    Ok(())
}
fn remove_children(directory: &SandboxDir, depth: usize) -> HbResult<()> {
    if depth > 32 {
        return Err(HbError::PayloadTooLarge);
    }
    let mut after = String::new();
    loop {
        // Finish enumeration before unlinking. Filesystems may otherwise skip names
        // when the enumerated directory changes underneath their iterator.
        let entries = directory.entries_page(&after, 256).map_err(local_error)?;
        if entries.is_empty() {
            break;
        }
        for (name, is_dir, is_file) in entries {
            after.clone_from(&name);
            if is_dir {
                remove_children(
                    &directory.child(&name, false).map_err(local_error)?,
                    depth + 1,
                )?;
            } else if is_file {
                drop(directory.read(&name).map_err(local_error)?);
                directory.remove(&name).map_err(local_error)?;
            } else {
                return Err(storage_error("non-regular entry in storage"));
            }
        }
    }
    Ok(())
}
