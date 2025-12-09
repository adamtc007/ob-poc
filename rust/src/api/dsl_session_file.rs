//! File-based DSL Session Storage
//!
//! Manages DSL sessions as actual files on disk, similar to how Claude Code
//! edits real source files. This enables:
//!
//! 1. Persistent DSL that survives server restarts
//! 2. Incremental editing - append statements, not regenerate
//! 3. Real parsing from files (like Zed/LSP does)
//! 4. Transparent audit trail of session evolution
//! 5. Full-file validation on each append (like LSP validates on save)
//!
//! ## Directory Structure
//!
//! ```text
//! /tmp/dsl-sessions/
//! └── {session_id}/
//!     ├── main.dsl           # Current accumulated DSL
//!     ├── history/
//!     │   ├── 001_initial.dsl
//!     │   ├── 002_added_entity.dsl
//!     │   └── ...
//!     └── session.json       # Session metadata
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

/// Base directory for DSL sessions
const DSL_SESSIONS_DIR: &str = "/tmp/dsl-sessions";

/// Metadata about a DSL session stored alongside the DSL file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslSessionMetadata {
    /// Session ID
    pub session_id: Uuid,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the DSL file was last modified
    pub updated_at: DateTime<Utc>,
    /// Number of statements in the DSL
    pub statement_count: usize,
    /// Domain hint for the session
    pub domain_hint: Option<String>,
    /// Named bindings created during the session (name -> UUID)
    pub bindings: HashMap<String, Uuid>,
    /// Last CBU ID created
    pub last_cbu_id: Option<Uuid>,
    /// Last entity ID created
    pub last_entity_id: Option<Uuid>,
    /// History of modifications
    pub history: Vec<DslHistoryEntry>,
}

/// A single modification to the DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslHistoryEntry {
    /// Sequential version number
    pub version: u32,
    /// When this change was made
    pub timestamp: DateTime<Utc>,
    /// What changed (user message that triggered it)
    pub description: String,
    /// Number of statements added
    pub statements_added: usize,
    /// Snapshot filename in history/ directory
    pub snapshot_file: String,
}

/// Manager for file-based DSL sessions
pub struct DslSessionFileManager {
    base_dir: PathBuf,
}

impl Default for DslSessionFileManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DslSessionFileManager {
    /// Create a new file manager with default base directory
    pub fn new() -> Self {
        Self {
            base_dir: PathBuf::from(DSL_SESSIONS_DIR),
        }
    }

    /// Create with custom base directory (useful for testing)
    pub fn with_base_dir(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    /// Get the directory path for a session
    fn session_dir(&self, session_id: Uuid) -> PathBuf {
        self.base_dir.join(session_id.to_string())
    }

    /// Get the main DSL file path for a session
    fn main_dsl_path(&self, session_id: Uuid) -> PathBuf {
        self.session_dir(session_id).join("main.dsl")
    }

    /// Get the metadata file path for a session
    fn metadata_path(&self, session_id: Uuid) -> PathBuf {
        self.session_dir(session_id).join("session.json")
    }

    /// Get the history directory for a session
    fn history_dir(&self, session_id: Uuid) -> PathBuf {
        self.session_dir(session_id).join("history")
    }

    /// Create a new DSL session with empty file
    pub async fn create_session(
        &self,
        session_id: Uuid,
        domain_hint: Option<String>,
    ) -> Result<DslSessionMetadata, std::io::Error> {
        let session_dir = self.session_dir(session_id);
        let history_dir = self.history_dir(session_id);

        // Create directories
        fs::create_dir_all(&session_dir).await?;
        fs::create_dir_all(&history_dir).await?;

        // Create empty main.dsl with header comment
        let header = format!(
            ";; DSL Session: {}\n;; Created: {}\n;; Domain: {}\n\n",
            session_id,
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            domain_hint.as_deref().unwrap_or("general")
        );
        fs::write(self.main_dsl_path(session_id), &header).await?;

        // Create metadata
        let metadata = DslSessionMetadata {
            session_id,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            statement_count: 0,
            domain_hint,
            bindings: HashMap::new(),
            last_cbu_id: None,
            last_entity_id: None,
            history: Vec::new(),
        };

        // Save metadata
        self.save_metadata(&metadata).await?;

        Ok(metadata)
    }

    /// Load session metadata
    pub async fn load_metadata(
        &self,
        session_id: Uuid,
    ) -> Result<DslSessionMetadata, std::io::Error> {
        let path = self.metadata_path(session_id);
        let content = fs::read_to_string(&path).await?;
        serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Save session metadata
    pub async fn save_metadata(&self, metadata: &DslSessionMetadata) -> Result<(), std::io::Error> {
        let path = self.metadata_path(metadata.session_id);
        let content = serde_json::to_string_pretty(metadata)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, content).await
    }

    /// Read the current DSL content
    pub async fn read_dsl(&self, session_id: Uuid) -> Result<String, std::io::Error> {
        fs::read_to_string(self.main_dsl_path(session_id)).await
    }

    /// Append DSL statements to the session file
    ///
    /// This is the primary way to add DSL - it appends rather than replaces,
    /// similar to how Claude Code adds code incrementally.
    pub async fn append_dsl(
        &self,
        session_id: Uuid,
        new_statements: &str,
        description: &str,
    ) -> Result<DslSessionMetadata, std::io::Error> {
        let mut metadata = self.load_metadata(session_id).await?;
        let dsl_path = self.main_dsl_path(session_id);

        // Read current content
        let mut current = fs::read_to_string(&dsl_path).await?;

        // Count new statements (rough heuristic: count opening parens at start of lines)
        let new_statement_count = new_statements
            .lines()
            .filter(|l| l.trim().starts_with('('))
            .count();

        // Add separator and new statements
        if !current.ends_with('\n') {
            current.push('\n');
        }
        current.push_str(&format!(
            "\n;; --- Added: {} ---\n{}\n",
            description, new_statements
        ));

        // Write updated content
        fs::write(&dsl_path, &current).await?;

        // Create history snapshot
        let version = metadata.history.len() as u32 + 1;
        let snapshot_file = format!("{:03}_{}.dsl", version, sanitize_filename(description));
        let snapshot_path = self.history_dir(session_id).join(&snapshot_file);
        fs::write(snapshot_path, &current).await?;

        // Update metadata
        metadata.updated_at = Utc::now();
        metadata.statement_count += new_statement_count;
        metadata.history.push(DslHistoryEntry {
            version,
            timestamp: Utc::now(),
            description: description.to_string(),
            statements_added: new_statement_count,
            snapshot_file,
        });

        self.save_metadata(&metadata).await?;

        Ok(metadata)
    }

    /// Replace the entire DSL content (use sparingly - prefer append)
    pub async fn write_dsl(
        &self,
        session_id: Uuid,
        dsl: &str,
        description: &str,
    ) -> Result<DslSessionMetadata, std::io::Error> {
        let mut metadata = self.load_metadata(session_id).await?;
        let dsl_path = self.main_dsl_path(session_id);

        // Count statements
        let statement_count = dsl.lines().filter(|l| l.trim().starts_with('(')).count();

        // Add header to DSL
        let content = format!(
            ";; DSL Session: {}\n;; Updated: {}\n;; Domain: {}\n\n{}",
            session_id,
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            metadata.domain_hint.as_deref().unwrap_or("general"),
            dsl
        );

        // Write content
        fs::write(&dsl_path, &content).await?;

        // Create history snapshot
        let version = metadata.history.len() as u32 + 1;
        let snapshot_file = format!("{:03}_{}.dsl", version, sanitize_filename(description));
        let snapshot_path = self.history_dir(session_id).join(&snapshot_file);
        fs::write(snapshot_path, &content).await?;

        // Update metadata
        metadata.updated_at = Utc::now();
        metadata.statement_count = statement_count;
        metadata.history.push(DslHistoryEntry {
            version,
            timestamp: Utc::now(),
            description: description.to_string(),
            statements_added: statement_count,
            snapshot_file,
        });

        self.save_metadata(&metadata).await?;

        Ok(metadata)
    }

    /// Update bindings after execution
    pub async fn update_bindings(
        &self,
        session_id: Uuid,
        bindings: &HashMap<String, Uuid>,
        last_cbu_id: Option<Uuid>,
        last_entity_id: Option<Uuid>,
    ) -> Result<(), std::io::Error> {
        let mut metadata = self.load_metadata(session_id).await?;

        // Merge bindings
        for (name, id) in bindings {
            metadata.bindings.insert(name.clone(), *id);
        }

        if let Some(id) = last_cbu_id {
            metadata.last_cbu_id = Some(id);
        }
        if let Some(id) = last_entity_id {
            metadata.last_entity_id = Some(id);
        }

        metadata.updated_at = Utc::now();
        self.save_metadata(&metadata).await
    }

    /// Check if a session exists
    pub async fn session_exists(&self, session_id: Uuid) -> bool {
        self.session_dir(session_id).exists()
    }

    /// Delete a session and all its files
    pub async fn delete_session(&self, session_id: Uuid) -> Result<(), std::io::Error> {
        let session_dir = self.session_dir(session_id);
        if session_dir.exists() {
            fs::remove_dir_all(session_dir).await?;
        }
        Ok(())
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Result<Vec<Uuid>, std::io::Error> {
        let mut sessions = Vec::new();

        if !self.base_dir.exists() {
            return Ok(sessions);
        }

        let mut entries = fs::read_dir(&self.base_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Ok(id) = Uuid::parse_str(&entry.file_name().to_string_lossy()) {
                    sessions.push(id);
                }
            }
        }

        Ok(sessions)
    }

    /// Get a specific history version
    pub async fn read_history_version(
        &self,
        session_id: Uuid,
        version: u32,
    ) -> Result<String, std::io::Error> {
        let metadata = self.load_metadata(session_id).await?;

        let entry = metadata
            .history
            .iter()
            .find(|h| h.version == version)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("History version {} not found", version),
                )
            })?;

        let path = self.history_dir(session_id).join(&entry.snapshot_file);
        fs::read_to_string(path).await
    }

    /// Revert to a previous version
    pub async fn revert_to_version(
        &self,
        session_id: Uuid,
        version: u32,
    ) -> Result<DslSessionMetadata, std::io::Error> {
        let historical_dsl = self.read_history_version(session_id, version).await?;
        self.write_dsl(
            session_id,
            &historical_dsl,
            &format!("Reverted to version {}", version),
        )
        .await
    }
}

/// Sanitize a string for use as a filename
fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(30) // Limit length
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_manager() -> (DslSessionFileManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = DslSessionFileManager::with_base_dir(temp_dir.path());
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_create_session() {
        let (manager, _temp) = create_test_manager().await;
        let session_id = Uuid::new_v4();

        let metadata = manager
            .create_session(session_id, Some("cbu".to_string()))
            .await
            .unwrap();

        assert_eq!(metadata.session_id, session_id);
        assert_eq!(metadata.domain_hint, Some("cbu".to_string()));
        assert_eq!(metadata.statement_count, 0);
        assert!(metadata.history.is_empty());
    }

    #[tokio::test]
    async fn test_append_dsl() {
        let (manager, _temp) = create_test_manager().await;
        let session_id = Uuid::new_v4();

        manager.create_session(session_id, None).await.unwrap();

        // First append
        let metadata = manager
            .append_dsl(
                session_id,
                r#"(cbu.ensure :name "Test" :jurisdiction "LU" :as @cbu)"#,
                "Create CBU",
            )
            .await
            .unwrap();

        assert_eq!(metadata.statement_count, 1);
        assert_eq!(metadata.history.len(), 1);

        // Second append
        let metadata = manager
            .append_dsl(
                session_id,
                r#"(entity.create-proper-person :first-name "John" :last-name "Doe" :as @person)"#,
                "Add person",
            )
            .await
            .unwrap();

        assert_eq!(metadata.statement_count, 2);
        assert_eq!(metadata.history.len(), 2);

        // Read DSL
        let dsl = manager.read_dsl(session_id).await.unwrap();
        assert!(dsl.contains("cbu.ensure"));
        assert!(dsl.contains("entity.create-proper-person"));
    }

    #[tokio::test]
    async fn test_history_and_revert() {
        let (manager, _temp) = create_test_manager().await;
        let session_id = Uuid::new_v4();

        manager.create_session(session_id, None).await.unwrap();

        // Make some changes
        manager
            .append_dsl(session_id, "(cbu.ensure :name \"V1\")", "Version 1")
            .await
            .unwrap();

        manager
            .append_dsl(session_id, "(cbu.ensure :name \"V2\")", "Version 2")
            .await
            .unwrap();

        // Read history version 1
        let v1 = manager.read_history_version(session_id, 1).await.unwrap();
        assert!(v1.contains("V1"));
        assert!(!v1.contains("V2"));

        // Revert to version 1
        manager.revert_to_version(session_id, 1).await.unwrap();

        let current = manager.read_dsl(session_id).await.unwrap();
        // After revert, current should be based on v1
        // (exact content depends on how revert writes)
        assert!(current.contains("V1"));
    }

    #[tokio::test]
    async fn test_bindings() {
        let (manager, _temp) = create_test_manager().await;
        let session_id = Uuid::new_v4();

        manager.create_session(session_id, None).await.unwrap();

        let cbu_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();

        let mut bindings = HashMap::new();
        bindings.insert("cbu".to_string(), cbu_id);
        bindings.insert("person".to_string(), entity_id);

        manager
            .update_bindings(session_id, &bindings, Some(cbu_id), Some(entity_id))
            .await
            .unwrap();

        let metadata = manager.load_metadata(session_id).await.unwrap();
        assert_eq!(metadata.bindings.get("cbu"), Some(&cbu_id));
        assert_eq!(metadata.bindings.get("person"), Some(&entity_id));
        assert_eq!(metadata.last_cbu_id, Some(cbu_id));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Create CBU"), "create_cbu");
        assert_eq!(sanitize_filename("Add person!"), "add_person_");
        assert_eq!(
            sanitize_filename("This is a very long description that should be truncated"),
            "this_is_a_very_long_descriptio"
        );
    }
}
