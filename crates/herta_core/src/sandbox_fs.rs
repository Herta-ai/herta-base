//! Directory-handle-relative I/O. All path components are opened without following links.

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, File, OpenOptions},
};
use std::{
    io,
    path::{Component, Path},
};

pub struct SandboxDir(Dir);

impl SandboxDir {
    pub fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }
    pub fn rename(&self, source: &str, destination: &Self, name: &str) -> io::Result<()> {
        validate_component(source)?;
        validate_component(name)?;
        self.0.rename(source, &destination.0, name)
    }
    pub fn open(path: &Path, create: bool) -> io::Result<Self> {
        let absolute = std::path::absolute(path)?;
        let mut root = std::path::PathBuf::new();
        let mut names = Vec::new();
        for component in absolute.components() {
            match component {
                Component::Prefix(_) | Component::RootDir => root.push(component.as_os_str()),
                Component::Normal(name) => names.push(name.to_owned()),
                Component::CurDir => {}
                Component::ParentDir => return Err(denied()),
            }
        }
        let mut directory = Dir::open_ambient_dir(root, ambient_authority())?;
        for name in names {
            if create {
                match directory.create_dir(&name) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error),
                }
            }
            directory = directory.open_dir_nofollow(name)?;
        }
        Ok(Self(directory))
    }

    pub fn child(&self, name: &str, create: bool) -> io::Result<Self> {
        validate_component(name)?;
        if create {
            match self.0.create_dir(name) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        self.0.open_dir_nofollow(name).map(Self)
    }

    pub fn entries(&self, limit: usize) -> io::Result<Vec<(String, bool, bool)>> {
        let entries: Vec<_> = self
            .iter()?
            .take(limit.saturating_add(1))
            .collect::<io::Result<_>>()?;
        if entries.len() > limit {
            return Err(io::Error::other("directory entry limit exceeded"));
        }
        Ok(entries)
    }

    /// Streams entries relative to this open directory; never resolves a child by an ambient path.
    pub fn iter(&self) -> io::Result<impl Iterator<Item = io::Result<(String, bool, bool)>> + '_> {
        Ok(self.0.entries()?.map(|entry| {
            let entry = entry?;
            let name = entry.file_name().into_string().map_err(|_| denied())?;
            let kind = entry.file_type()?;
            Ok((name, kind.is_dir(), kind.is_file()))
        }))
    }

    /// Keeps only the next `limit` names, independent of filesystem enumeration order.
    /// The cursor remains usable after deleting the returned entries.
    pub fn entries_page(&self, after: &str, limit: usize) -> io::Result<Vec<(String, bool, bool)>> {
        if !(1..=10000).contains(&limit) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid page size",
            ));
        }
        let mut entries = std::collections::BTreeMap::new();
        for entry in self.iter()? {
            let (name, is_dir, is_file) = entry?;
            if name.as_str() <= after {
                continue;
            }
            entries.insert(name, (is_dir, is_file));
            if entries.len() > limit {
                entries.pop_last();
            }
        }
        Ok(entries
            .into_iter()
            .map(|(name, (dir, file))| (name, dir, file))
            .collect())
    }

    pub fn read(&self, name: &str) -> io::Result<File> {
        validate_component(name)?;
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let file = self.0.open_with(name, &options)?;
        if !file.metadata()?.is_file() {
            return Err(denied());
        }
        Ok(file)
    }

    pub fn create_new(&self, name: &str) -> io::Result<File> {
        validate_component(name)?;
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        self.0.open_with(name, &options)
    }

    pub fn remove(&self, name: &str) -> io::Result<()> {
        validate_component(name)?;
        self.0.remove_file(name)
    }
}

fn denied() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "sandbox path rejected")
}

pub fn validate_component(name: &str) -> io::Result<()> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':', '\0'])
        || name.ends_with(['.', ' '])
        || name.chars().any(char::is_control)
        || matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        )
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
        })
    {
        return Err(denied());
    }
    Ok(())
}
