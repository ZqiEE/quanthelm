//! Append-only JSONL persistence used for audit and deterministic replay.

use std::path::{Path, PathBuf};

use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};

/// Storage failure with line context for replay errors.
#[derive(Debug, Error)]
pub enum StorageError {
    /// File-system operation failed.
    #[error("storage I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Serialization failed before append.
    #[error("failed to serialize JSONL record: {0}")]
    Serialize(#[source] serde_json::Error),
    /// A specific JSONL line could not be decoded.
    #[error("invalid JSONL at line {line}: {source}")]
    Deserialize {
        /// One-based line number.
        line: usize,
        /// JSON decoder failure.
        #[source]
        source: serde_json::Error,
    },
}

/// Append-only newline-delimited JSON store.
#[derive(Debug, Clone)]
pub struct JsonlStore {
    path: PathBuf,
}

impl JsonlStore {
    /// Creates a store for the provided path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the backing path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Appends one complete record and syncs it to the file system.
    pub async fn append<T>(&self, value: &T) -> Result<(), StorageError>
    where
        T: Serialize + ?Sized,
    {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).await?;
        }

        let mut encoded = serde_json::to_vec(value).map_err(StorageError::Serialize)?;
        encoded.push(b'\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        file.write_all(&encoded).await?;
        file.sync_data().await?;
        Ok(())
    }

    /// Reads all records in deterministic file order.
    pub async fn read_all<T>(&self) -> Result<Vec<T>, StorageError>
    where
        T: DeserializeOwned,
    {
        let content = fs::read_to_string(&self.path).await?;
        content
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(index, line)| {
                serde_json::from_str(line).map_err(|source| StorageError::Deserialize {
                    line: index + 1,
                    source,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    #[tokio::test]
    async fn appends_and_replays_in_order() {
        let path = std::env::temp_dir().join(format!("quanthelm-{}.jsonl", Uuid::new_v4()));
        let store = JsonlStore::new(&path);

        store.append(&1_u64).await.expect("append first");
        store.append(&2_u64).await.expect("append second");

        let values: Vec<u64> = store.read_all().await.expect("read records");
        assert_eq!(values, vec![1, 2]);

        fs::remove_file(path).await.expect("remove fixture");
    }
}
