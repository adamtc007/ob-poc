//! Blob Storage Abstraction
//!
//! Abstract interface for storing document binaries.
//! Implementations can target local filesystem (POC) or S3-compatible storage (production).

use async_trait::async_trait;
use std::path::PathBuf;

/// Error type for blob storage operations
#[derive(Debug, thiserror::Error)]
pub enum BlobStoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid blob reference: {0}")]
    InvalidRef(String),

    #[error("Blob not found: {0}")]
    NotFound(String),

    #[error("Storage error: {0}")]
    Storage(String),
}

/// Abstract blob storage for document binaries
#[async_trait]
pub trait BlobStore: Send + Sync {
    /// Store binary content, return reference URI
    async fn store(
        &self,
        key: &str,
        content: &[u8],
        content_type: &str,
    ) -> Result<String, BlobStoreError>;

    /// Fetch binary content by reference
    async fn fetch(&self, blob_ref: &str) -> Result<Vec<u8>, BlobStoreError>;

    /// Delete binary content
    async fn delete(&self, blob_ref: &str) -> Result<(), BlobStoreError>;

    /// Generate presigned URL for direct access (optional)
    async fn presigned_url(
        &self,
        _blob_ref: &str,
        _expires_secs: u64,
    ) -> Result<Option<String>, BlobStoreError> {
        Ok(None) // Default: not supported
    }

    /// Check if blob exists
    async fn exists(&self, blob_ref: &str) -> Result<bool, BlobStoreError>;
}

/// Local filesystem implementation (for POC)
pub struct LocalBlobStore {
    base_path: PathBuf,
}

impl LocalBlobStore {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Get the full path for a key
    fn path_for_key(&self, key: &str) -> PathBuf {
        self.base_path.join(key)
    }

    /// Extract path from blob_ref (file:// URI)
    fn path_from_ref(&self, blob_ref: &str) -> Result<PathBuf, BlobStoreError> {
        blob_ref
            .strip_prefix("file://")
            .map(PathBuf::from)
            .ok_or_else(|| {
                BlobStoreError::InvalidRef(format!("Expected file:// prefix: {}", blob_ref))
            })
    }
}

#[async_trait]
impl BlobStore for LocalBlobStore {
    async fn store(
        &self,
        key: &str,
        content: &[u8],
        _content_type: &str,
    ) -> Result<String, BlobStoreError> {
        let path = self.path_for_key(key);

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&path, content).await?;
        Ok(format!("file://{}", path.display()))
    }

    async fn fetch(&self, blob_ref: &str) -> Result<Vec<u8>, BlobStoreError> {
        let path = self.path_from_ref(blob_ref)?;

        if !path.exists() {
            return Err(BlobStoreError::NotFound(blob_ref.to_string()));
        }

        Ok(tokio::fs::read(path).await?)
    }

    async fn delete(&self, blob_ref: &str) -> Result<(), BlobStoreError> {
        let path = self.path_from_ref(blob_ref)?;

        if path.exists() {
            tokio::fs::remove_file(path).await?;
        }

        Ok(())
    }

    async fn exists(&self, blob_ref: &str) -> Result<bool, BlobStoreError> {
        let path = self.path_from_ref(blob_ref)?;
        Ok(path.exists())
    }
}

/// In-memory blob store (for testing)
#[cfg(test)]
pub struct InMemoryBlobStore {
    blobs: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, Vec<u8>>>>,
}

#[cfg(test)]
impl InMemoryBlobStore {
    pub fn new() -> Self {
        Self {
            blobs: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl BlobStore for InMemoryBlobStore {
    async fn store(
        &self,
        key: &str,
        content: &[u8],
        _content_type: &str,
    ) -> Result<String, BlobStoreError> {
        let blob_ref = format!("memory://{}", key);
        let mut blobs = self.blobs.write().await;
        blobs.insert(blob_ref.clone(), content.to_vec());
        Ok(blob_ref)
    }

    async fn fetch(&self, blob_ref: &str) -> Result<Vec<u8>, BlobStoreError> {
        let blobs = self.blobs.read().await;
        blobs
            .get(blob_ref)
            .cloned()
            .ok_or_else(|| BlobStoreError::NotFound(blob_ref.to_string()))
    }

    async fn delete(&self, blob_ref: &str) -> Result<(), BlobStoreError> {
        let mut blobs = self.blobs.write().await;
        blobs.remove(blob_ref);
        Ok(())
    }

    async fn exists(&self, blob_ref: &str) -> Result<bool, BlobStoreError> {
        let blobs = self.blobs.read().await;
        Ok(blobs.contains_key(blob_ref))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_local_blob_store_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalBlobStore::new(temp_dir.path());

        let content = b"Hello, World!";
        let key = "test/document.pdf";

        // Store
        let blob_ref = store.store(key, content, "application/pdf").await.unwrap();
        assert!(blob_ref.starts_with("file://"));

        // Exists
        assert!(store.exists(&blob_ref).await.unwrap());

        // Fetch
        let fetched = store.fetch(&blob_ref).await.unwrap();
        assert_eq!(fetched, content);

        // Delete
        store.delete(&blob_ref).await.unwrap();
        assert!(!store.exists(&blob_ref).await.unwrap());
    }

    #[tokio::test]
    async fn test_local_blob_store_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalBlobStore::new(temp_dir.path());

        let content = b"Nested content";
        let key = "a/b/c/deep/file.txt";

        let blob_ref = store.store(key, content, "text/plain").await.unwrap();
        let fetched = store.fetch(&blob_ref).await.unwrap();
        assert_eq!(fetched, content);
    }

    #[tokio::test]
    async fn test_in_memory_blob_store() {
        let store = InMemoryBlobStore::new();

        let content = b"Test data";
        let key = "test-key";

        let blob_ref = store
            .store(key, content, "application/octet-stream")
            .await
            .unwrap();
        assert!(store.exists(&blob_ref).await.unwrap());

        let fetched = store.fetch(&blob_ref).await.unwrap();
        assert_eq!(fetched, content);

        store.delete(&blob_ref).await.unwrap();
        assert!(!store.exists(&blob_ref).await.unwrap());
    }

    #[tokio::test]
    async fn test_not_found_error() {
        let store = InMemoryBlobStore::new();
        let result = store.fetch("memory://nonexistent").await;
        assert!(matches!(result, Err(BlobStoreError::NotFound(_))));
    }
}
