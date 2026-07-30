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
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

/// Base directory for DSL sessions
const DSL_SESSIONS_DIR: &str = "/tmp/dsl-sessions";

/// Metadata about a DSL session stored alongside the DSL file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DslSessionMetadata {
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
pub(crate) struct DslHistoryEntry {
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
pub(crate) struct DslSessionFileManager {
    base_dir: PathBuf,
}

impl Default for DslSessionFileManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DslSessionFileManager {
    /// Create a new file manager with default base directory
    pub(crate) fn new() -> Self {
        Self {
            base_dir: PathBuf::from(DSL_SESSIONS_DIR),
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

    /// Load session metadata
    pub(crate) async fn load_metadata(
        &self,
        session_id: Uuid,
    ) -> Result<DslSessionMetadata, std::io::Error> {
        let path = self.metadata_path(session_id);
        let content = fs::read_to_string(&path).await?;
        serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Save session metadata
    pub(crate) async fn save_metadata(&self, metadata: &DslSessionMetadata) -> Result<(), std::io::Error> {
        let path = self.metadata_path(metadata.session_id);
        let content = serde_json::to_string_pretty(metadata)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, content).await
    }

    /// Append DSL statements to the session file
    ///
    /// This is the primary way to add DSL - it appends rather than replaces,
    /// similar to how Claude Code adds code incrementally.
    pub(crate) async fn append_dsl(
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

    /// Directly construct a manager pointed at a temp dir and seed the
    /// on-disk state that `append_dsl` expects (empty main.dsl + metadata
    /// file), bypassing the now-deleted `create_session`/`with_base_dir`
    /// convenience wrappers.
    async fn create_test_manager() -> (DslSessionFileManager, TempDir, Uuid) {
        let temp_dir = TempDir::new().unwrap();
        let manager = DslSessionFileManager {
            base_dir: temp_dir.path().to_path_buf(),
        };
        let session_id = Uuid::new_v4();

        fs::create_dir_all(manager.session_dir(session_id))
            .await
            .unwrap();
        fs::create_dir_all(manager.history_dir(session_id))
            .await
            .unwrap();
        fs::write(manager.main_dsl_path(session_id), "")
            .await
            .unwrap();
        manager
            .save_metadata(&DslSessionMetadata {
                session_id,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                statement_count: 0,
                domain_hint: None,
                bindings: HashMap::new(),
                last_cbu_id: None,
                last_entity_id: None,
                history: Vec::new(),
            })
            .await
            .unwrap();

        (manager, temp_dir, session_id)
    }

    #[tokio::test]
    async fn test_append_dsl() {
        let (manager, _temp, session_id) = create_test_manager().await;

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
                r#"(entity.create :entity-type "proper-person" :first-name "John" :last-name "Doe" :as @person)"#,
                "Add person",
            )
            .await
            .unwrap();

        assert_eq!(metadata.statement_count, 2);
        assert_eq!(metadata.history.len(), 2);

        // Verify file content directly (read_dsl was dead code, removed)
        let dsl = fs::read_to_string(manager.main_dsl_path(session_id))
            .await
            .unwrap();
        assert!(dsl.contains("cbu.ensure"));
        assert!(dsl.contains("entity.create"));
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
