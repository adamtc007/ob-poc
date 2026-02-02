//! Event storage backends.
//!
//! This module provides append-only event storage. The primary implementation
//! uses JSONL (JSON Lines) files for simplicity and grep-ability. A database
//! backend is also available for production use.
//!
//! The store is designed for write-heavy workloads:
//! - Append-only (no updates or deletes)
//! - Batched writes for efficiency
//! - Minimal indexing (just timestamp)

use std::path::PathBuf;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tracing::debug;

use super::types::DslEvent;

/// Append-only event store.
///
/// Uses JSONL format (one JSON object per line) for simplicity.
/// This format is:
/// - Human-readable
/// - Grep-friendly
/// - Easy to parse line by line
/// - Append-only (no locking needed for single writer)
#[derive(Clone)]
pub struct EventStore {
    path: PathBuf,
}

impl EventStore {
    /// Create a new file-based event store.
    ///
    /// The file will be created if it doesn't exist.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Append an event to the store.
    ///
    /// Each event is written as a single JSON line followed by newline.
    pub async fn append(&self, event: &DslEvent) -> Result<(), std::io::Error> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;

        let line = serde_json::to_string(event)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;

        Ok(())
    }

    /// Append multiple events in a single write (more efficient).
    pub async fn append_batch(&self, events: &[DslEvent]) -> Result<(), std::io::Error> {
        if events.is_empty() {
            return Ok(());
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;

        let mut buffer = String::new();
        for event in events {
            let line = serde_json::to_string(event)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            buffer.push_str(&line);
            buffer.push('\n');
        }

        file.write_all(buffer.as_bytes()).await?;

        debug!(count = events.len(), "Wrote event batch to store");

        Ok(())
    }

    /// Flush the store to disk.
    ///
    /// For file-based stores with append mode, each write is already flushed.
    /// This is a hook for future optimizations (buffered writes).
    pub async fn flush(&self) -> Result<(), std::io::Error> {
        // With append mode, each write is already flushed to OS
        // This is here for future batching optimization
        Ok(())
    }

    /// Get the store path.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Read all events from the store (for testing/debugging).
    ///
    /// Returns events in chronological order.
    pub async fn read_all(&self) -> Result<Vec<DslEvent>, std::io::Error> {
        let content = tokio::fs::read_to_string(&self.path).await?;

        let mut events = Vec::new();
        for line in content.lines() {
            if line.is_empty() {
                continue;
            }
            let event: DslEvent = serde_json::from_str(line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            events.push(event);
        }

        Ok(events)
    }

    /// Count events in the store.
    pub async fn count(&self) -> Result<usize, std::io::Error> {
        match tokio::fs::read_to_string(&self.path).await {
            Ok(content) => Ok(content.lines().filter(|l| !l.is_empty()).count()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(e) => Err(e),
        }
    }
}

/// Database-backed event store.
///
/// For production use where events need to be queryable.
#[cfg(feature = "database")]
pub struct DbEventStore {
    pool: sqlx::PgPool,
}

#[cfg(feature = "database")]
impl DbEventStore {
    /// Create a new database-backed event store.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Append an event to the database.
    pub async fn append(&self, event: &DslEvent) -> Result<(), sqlx::Error> {
        let payload = serde_json::to_value(&event.payload).unwrap_or_default();

        sqlx::query!(
            r#"
            INSERT INTO events.log (timestamp, session_id, event_type, payload)
            VALUES ($1, $2, $3, $4)
            "#,
            event.timestamp,
            event.session_id,
            event.payload.event_type_str(),
            payload,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Append multiple events in a batch.
    pub async fn append_batch(&self, events: &[DslEvent]) -> Result<(), sqlx::Error> {
        for event in events {
            self.append(event).await?;
        }
        Ok(())
    }

    /// Flush (no-op for database).
    pub async fn flush(&self) -> Result<(), sqlx::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::{EventPayload, SessionSource};
    use tempfile::tempdir;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_append_and_read() {
        let dir = tempdir().unwrap();
        let store = EventStore::new(dir.path().join("events.jsonl"));

        let event = DslEvent::succeeded(Some(Uuid::now_v7()), "test.verb".to_string(), 100);

        store.append(&event).await.unwrap();
        store.flush().await.unwrap();

        let events = store.read_all().await.unwrap();
        assert_eq!(events.len(), 1);

        match &events[0].payload {
            EventPayload::CommandSucceeded { verb, duration_ms } => {
                assert_eq!(verb, "test.verb");
                assert_eq!(*duration_ms, 100);
            }
            _ => panic!("Expected CommandSucceeded"),
        }
    }

    #[tokio::test]
    async fn test_append_batch() {
        let dir = tempdir().unwrap();
        let store = EventStore::new(dir.path().join("events.jsonl"));

        let events: Vec<DslEvent> = (0..10)
            .map(|i| DslEvent::succeeded(None, format!("verb.{}", i), i as u64))
            .collect();

        store.append_batch(&events).await.unwrap();

        let read_events = store.read_all().await.unwrap();
        assert_eq!(read_events.len(), 10);
    }

    #[tokio::test]
    async fn test_count() {
        let dir = tempdir().unwrap();
        let store = EventStore::new(dir.path().join("events.jsonl"));

        assert_eq!(store.count().await.unwrap(), 0);

        for i in 0..5 {
            store
                .append(&DslEvent::succeeded(None, format!("v{}", i), i as u64))
                .await
                .unwrap();
        }

        assert_eq!(store.count().await.unwrap(), 5);
    }

    #[tokio::test]
    async fn test_session_events() {
        let dir = tempdir().unwrap();
        let store = EventStore::new(dir.path().join("events.jsonl"));

        let session_id = Uuid::now_v7();

        store
            .append(&DslEvent::session_started(session_id, SessionSource::Repl))
            .await
            .unwrap();

        store
            .append(&DslEvent::session_ended(session_id, 10, 2, 60))
            .await
            .unwrap();

        let events = store.read_all().await.unwrap();
        assert_eq!(events.len(), 2);

        match &events[0].payload {
            EventPayload::SessionStarted { source } => {
                assert_eq!(*source, SessionSource::Repl);
            }
            _ => panic!("Expected SessionStarted"),
        }

        match &events[1].payload {
            EventPayload::SessionEnded {
                command_count,
                error_count,
                duration_secs,
            } => {
                assert_eq!(*command_count, 10);
                assert_eq!(*error_count, 2);
                assert_eq!(*duration_secs, 60);
            }
            _ => panic!("Expected SessionEnded"),
        }
    }
}
