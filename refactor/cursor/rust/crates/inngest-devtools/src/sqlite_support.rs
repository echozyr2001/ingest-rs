//! SQLite support for development environment
//!
//! This module provides SQLite-based implementations for state management,
//! compatible with both in-memory and file-based SQLite databases.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use inngest_core::Result;
use inngest_state::{
    CreateStateInput, Metadata, MetadataUpdate, RunStatus, State, StateId,
    StateManager,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
    Row, SqlitePool,
};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

/// SQLite database wrapper
pub struct SqliteDatabase {
    pool: SqlitePool,
}

impl SqliteDatabase {
    /// Create a new in-memory SQLite database
    pub async fn new_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite://:memory:")
            .map_err(|e| inngest_core::Error::Database(e.to_string()))?
            .journal_mode(SqliteJournalMode::Memory)
            .create_if_missing(true);

        let pool = SqlitePool::connect_with(options).await.map_err(|e| {
            inngest_core::Error::Database(format!("Failed to create SQLite pool: {}", e))
        })?;

        let db = Self { pool };
        db.initialize_schema().await?;
        Ok(db)
    }

    /// Create a new file-based SQLite database
    pub async fn new_file(path: &str) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path))
            .map_err(|e| inngest_core::Error::Database(e.to_string()))?
            .journal_mode(SqliteJournalMode::Wal)
            .create_if_missing(true);

        let pool = SqlitePool::connect_with(options).await.map_err(|e| {
            inngest_core::Error::Database(format!("Failed to create SQLite pool: {}", e))
        })?;

        let db = Self { pool };
        db.initialize_schema().await?;
        Ok(db)
    }

    /// Initialize database schema
    async fn initialize_schema(&self) -> Result<()> {
        // Create tables for state management
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS function_runs (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                function_id TEXT NOT NULL,
                account_id TEXT NOT NULL,
                env_id TEXT NOT NULL,
                app_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'RUNNING',
                function_version INTEGER NOT NULL DEFAULT 1,
                event_id TEXT NOT NULL,
                event_ids TEXT NOT NULL, -- JSON array
                batch_id TEXT,
                original_run_id TEXT,
                started_at TEXT NOT NULL, -- ISO 8601
                debugger INTEGER NOT NULL DEFAULT 0,
                run_type TEXT,
                request_version INTEGER NOT NULL DEFAULT 1,
                context TEXT DEFAULT '{}', -- JSON
                span_id TEXT,
                idempotency_key TEXT,
                priority_factor INTEGER,
                events TEXT NOT NULL, -- JSON array
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            inngest_core::Error::Database(format!("Failed to create function_runs table: {}", e))
        })?;

        // Create table for step data
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS function_steps (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                step_id TEXT NOT NULL,
                step_data TEXT NOT NULL, -- JSON
                step_order INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (run_id) REFERENCES function_runs (id) ON DELETE CASCADE,
                UNIQUE(run_id, step_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            inngest_core::Error::Database(format!("Failed to create function_steps table: {}", e))
        })?;

        // Create table for step inputs
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS function_step_inputs (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                step_id TEXT NOT NULL,
                input_data TEXT NOT NULL, -- JSON
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (run_id) REFERENCES function_runs (id) ON DELETE CASCADE,
                UNIQUE(run_id, step_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            inngest_core::Error::Database(format!(
                "Failed to create function_step_inputs table: {}",
                e
            ))
        })?;

        // Create indexes
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_function_runs_run_id ON function_runs (run_id)",
        )
        .execute(&self.pool)
        .await
        .ok();
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_function_runs_function_id ON function_runs (function_id)")
            .execute(&self.pool).await.ok();
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_function_runs_status ON function_runs (status)",
        )
        .execute(&self.pool)
        .await
        .ok();
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_function_steps_run_id ON function_steps (run_id)",
        )
        .execute(&self.pool)
        .await
        .ok();
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_function_step_inputs_run_id ON function_step_inputs (run_id)")
            .execute(&self.pool).await.ok();

        Ok(())
    }

    /// Get the connection pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Truncate all tables (for testing)
    pub async fn truncate_all(&self) -> Result<()> {
        sqlx::query("DELETE FROM function_step_inputs")
            .execute(&self.pool)
            .await
            .ok();
        sqlx::query("DELETE FROM function_steps")
            .execute(&self.pool)
            .await
            .ok();
        sqlx::query("DELETE FROM function_runs")
            .execute(&self.pool)
            .await
            .ok();
        Ok(())
    }

    /// Export data as a snapshot
    pub async fn export_data(&self) -> Result<SqliteSnapshot> {
        let runs =
            sqlx::query_as::<_, SqliteRunRow>("SELECT * FROM function_runs ORDER BY created_at")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| {
                    inngest_core::Error::Database(format!("Failed to export runs: {}", e))
                })?;

        let steps = sqlx::query_as::<_, SqliteStepRow>(
            "SELECT * FROM function_steps ORDER BY run_id, step_order",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| inngest_core::Error::Database(format!("Failed to export steps: {}", e)))?;

        let step_inputs = sqlx::query_as::<_, SqliteStepInputRow>(
            "SELECT * FROM function_step_inputs ORDER BY run_id, step_id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            inngest_core::Error::Database(format!("Failed to export step inputs: {}", e))
        })?;

        Ok(SqliteSnapshot {
            runs,
            steps,
            step_inputs,
        })
    }

    /// Import data from a snapshot
    pub async fn import_data(&self, snapshot: SqliteSnapshot) -> Result<()> {
        self.truncate_all().await?;

        // Import runs
        for run in snapshot.runs {
            sqlx::query(
                r#"
                INSERT INTO function_runs (
                    id, run_id, function_id, account_id, env_id, app_id, status,
                    function_version, event_id, event_ids, batch_id, original_run_id,
                    started_at, debugger, run_type, request_version, context,
                    span_id, idempotency_key, priority_factor, events, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&run.id)
            .bind(&run.run_id)
            .bind(&run.function_id)
            .bind(&run.account_id)
            .bind(&run.env_id)
            .bind(&run.app_id)
            .bind(&run.status)
            .bind(run.function_version)
            .bind(&run.event_id)
            .bind(&run.event_ids)
            .bind(run.batch_id.as_ref())
            .bind(run.original_run_id.as_ref())
            .bind(&run.started_at)
            .bind(run.debugger)
            .bind(run.run_type.as_ref())
            .bind(run.request_version)
            .bind(&run.context)
            .bind(run.span_id.as_ref())
            .bind(run.idempotency_key.as_ref())
            .bind(run.priority_factor)
            .bind(&run.events)
            .bind(&run.created_at)
            .bind(&run.updated_at)
            .execute(&self.pool)
            .await
            .map_err(|e| inngest_core::Error::Database(format!("Failed to import run: {}", e)))?;
        }

        // Import steps
        for step in snapshot.steps {
            sqlx::query(
                "INSERT INTO function_steps (id, run_id, step_id, step_data, step_order, created_at) VALUES (?, ?, ?, ?, ?, ?)"
            )
            .bind(&step.id)
            .bind(&step.run_id)
            .bind(&step.step_id)
            .bind(&step.step_data)
            .bind(step.step_order)
            .bind(&step.created_at)
            .execute(&self.pool)
            .await
            .map_err(|e| inngest_core::Error::Database(format!("Failed to import step: {}", e)))?;
        }

        // Import step inputs
        for input in snapshot.step_inputs {
            sqlx::query(
                "INSERT INTO function_step_inputs (id, run_id, step_id, input_data, created_at) VALUES (?, ?, ?, ?, ?)"
            )
            .bind(&input.id)
            .bind(&input.run_id)
            .bind(&input.step_id)
            .bind(&input.input_data)
            .bind(&input.created_at)
            .execute(&self.pool)
            .await
            .map_err(|e| inngest_core::Error::Database(format!("Failed to import step input: {}", e)))?;
        }

        Ok(())
    }
}

/// SQLite-based state manager
pub struct SqliteStateManager {
    db: Arc<SqliteDatabase>,
}

impl SqliteStateManager {
    pub fn new(db: Arc<SqliteDatabase>) -> Self {
        Self { db }
    }

    fn state_key(id: &StateId) -> String {
        format!("{}:{}:{}", id.tenant.account_id, id.function_id, id.run_id)
    }
}

#[async_trait]
impl StateManager for SqliteStateManager {
    async fn create(&self, input: CreateStateInput) -> Result<State> {
        let state_key = Self::state_key(&input.id);

        // Check if state already exists
        let existing = sqlx::query("SELECT id FROM function_runs WHERE id = ?")
            .bind(&state_key)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| {
                inngest_core::Error::Database(format!("Failed to check existing state: {}", e))
            })?;

        if existing.is_some() {
            // State already exists, load and return it
            return self.load(&input.id).await?.ok_or_else(|| {
                inngest_core::Error::Database("State exists but could not be loaded".to_string())
            });
        }

        // Serialize data
        let event_ids_json = serde_json::to_string(&input.metadata.event_ids).map_err(|e| {
            inngest_core::Error::InvalidData(format!("Failed to serialize event IDs: {}", e))
        })?;

        let context_json = serde_json::to_string(&input.metadata.context).map_err(|e| {
            inngest_core::Error::InvalidData(format!("Failed to serialize context: {}", e))
        })?;

        let events_json = serde_json::to_string(&input.events).map_err(|e| {
            inngest_core::Error::InvalidData(format!("Failed to serialize events: {}", e))
        })?;

        // Create function run
        sqlx::query(
            r#"
            INSERT INTO function_runs (
                id, run_id, function_id, account_id, env_id, app_id, status,
                function_version, event_id, event_ids, batch_id, original_run_id,
                started_at, debugger, run_type, request_version, context,
                span_id, idempotency_key, priority_factor, events
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&state_key)
        .bind(input.id.run_id.to_string())
        .bind(input.id.function_id.to_string())
        .bind(input.id.tenant.account_id.to_string())
        .bind(input.id.tenant.env_id.to_string())
        .bind(input.id.tenant.app_id.to_string())
        .bind(input.metadata.status.to_string())
        .bind(input.metadata.function_version)
        .bind(input.metadata.event_id.to_string())
        .bind(&event_ids_json)
        .bind(input.metadata.batch_id.map(|id| id.to_string()))
        .bind(input.metadata.original_run_id.map(|id| id.to_string()))
        .bind(input.metadata.started_at.to_rfc3339())
        .bind(if input.metadata.debugger { 1 } else { 0 })
        .bind(&input.metadata.run_type)
        .bind(input.metadata.request_version)
        .bind(&context_json)
        .bind(&input.metadata.span_id)
        .bind(&input.metadata.key)
        .bind(input.metadata.priority_factor)
        .bind(&events_json)
        .execute(self.db.pool())
        .await
        .map_err(|e| inngest_core::Error::Database(format!("Failed to create state: {}", e)))?;

        // Add pre-memoized steps
        for (order, step) in input.steps.iter().enumerate() {
            let step_data_json = serde_json::to_string(&step.data).map_err(|e| {
                inngest_core::Error::InvalidData(format!("Failed to serialize step data: {}", e))
            })?;

            sqlx::query(
                "INSERT INTO function_steps (id, run_id, step_id, step_data, step_order) VALUES (?, ?, ?, ?, ?)"
            )
            .bind(format!("{}:{}", state_key, step.id))
            .bind(&state_key)
            .bind(&step.id)
            .bind(&step_data_json)
            .bind(order as i64)
            .execute(self.db.pool())
            .await
            .map_err(|e| inngest_core::Error::Database(format!("Failed to save step: {}", e)))?;
        }

        // Add step inputs
        for input_step in &input.step_inputs {
            let input_data_json = serde_json::to_string(&input_step.data).map_err(|e| {
                inngest_core::Error::InvalidData(format!("Failed to serialize step input: {}", e))
            })?;

            sqlx::query(
                "INSERT INTO function_step_inputs (id, run_id, step_id, input_data) VALUES (?, ?, ?, ?)"
            )
            .bind(format!("{}:{}", state_key, input_step.id))
            .bind(&state_key)
            .bind(&input_step.id)
            .bind(&input_data_json)
            .execute(self.db.pool())
            .await
            .map_err(|e| inngest_core::Error::Database(format!("Failed to save step input: {}", e)))?;
        }

        Ok(State::new(input))
    }

    async fn load(&self, id: &StateId) -> Result<Option<State>> {
        let state_key = Self::state_key(id);

        // Load metadata
        let row = sqlx::query("SELECT * FROM function_runs WHERE id = ?")
            .bind(&state_key)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| inngest_core::Error::Database(format!("Failed to load state: {}", e)))?;

        let Some(row) = row else {
            return Ok(None);
        };

        // Parse metadata
        let event_ids: Vec<String> = serde_json::from_str(row.get("event_ids")).map_err(|e| {
            inngest_core::Error::InvalidData(format!("Failed to parse event IDs: {}", e))
        })?;

        let context: HashMap<String, serde_json::Value> = serde_json::from_str(row.get("context"))
            .map_err(|e| {
                inngest_core::Error::InvalidData(format!("Failed to parse context: {}", e))
            })?;

        let events: Vec<serde_json::Value> =
            serde_json::from_str(row.get("events")).map_err(|e| {
                inngest_core::Error::InvalidData(format!("Failed to parse events: {}", e))
            })?;

        let metadata = Metadata {
            id: id.clone(),
            status: row
                .get::<String, _>("status")
                .parse()
                .unwrap_or(RunStatus::Running),
            function_version: row.get("function_version"),
            event_id: row.get::<String, _>("event_id").parse().map_err(|e| {
                inngest_core::Error::InvalidData(format!("Invalid event ID: {}", e))
            })?,
            event_ids: event_ids
                .into_iter()
                .filter_map(|s| s.parse().ok())
                .collect(),
            batch_id: row
                .get::<Option<String>, _>("batch_id")
                .and_then(|s| s.parse().ok()),
            original_run_id: row
                .get::<Option<String>, _>("original_run_id")
                .and_then(|s| s.parse().ok()),
            started_at: DateTime::parse_from_rfc3339(row.get("started_at"))
                .map_err(|e| inngest_core::Error::InvalidData(format!("Invalid date: {}", e)))?
                .with_timezone(&Utc),
            debugger: row.get::<i64, _>("debugger") != 0,
            run_type: row.get("run_type"),
            request_version: row.get("request_version"),
            context,
            span_id: row.get("span_id"),
            key: row.get("idempotency_key"),
            priority_factor: row.get("priority_factor"),
        };

        // Load steps
        let step_rows = sqlx::query(
            "SELECT step_id, step_data FROM function_steps WHERE run_id = ? ORDER BY step_order",
        )
        .bind(&state_key)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| inngest_core::Error::Database(format!("Failed to load steps: {}", e)))?;

        let mut steps = HashMap::new();
        let mut stack = Vec::new();

        for row in step_rows {
            let step_id: String = row.get("step_id");
            let step_data: serde_json::Value =
                serde_json::from_str(row.get("step_data")).map_err(|e| {
                    inngest_core::Error::InvalidData(format!("Failed to parse step data: {}", e))
                })?;

            steps.insert(step_id.clone(), step_data);
            stack.push(step_id);
        }

        // Load step inputs
        let input_rows =
            sqlx::query("SELECT step_id, input_data FROM function_step_inputs WHERE run_id = ?")
                .bind(&state_key)
                .fetch_all(self.db.pool())
                .await
                .map_err(|e| {
                    inngest_core::Error::Database(format!("Failed to load step inputs: {}", e))
                })?;

        let mut step_inputs = HashMap::new();
        for row in input_rows {
            let step_id: String = row.get("step_id");
            let input_data: serde_json::Value = serde_json::from_str(row.get("input_data"))
                .map_err(|e| {
                    inngest_core::Error::InvalidData(format!("Failed to parse step input: {}", e))
                })?;

            step_inputs.insert(step_id, input_data);
        }

        Ok(Some(State {
            metadata,
            events,
            stack,
            steps,
            step_inputs,
        }))
    }

    async fn load_metadata(&self, id: &StateId) -> Result<Option<Metadata>> {
        let state = self.load(id).await?;
        Ok(state.map(|s| s.metadata))
    }

    async fn update_metadata(&self, id: &StateId, update: MetadataUpdate) -> Result<()> {
        let state_key = Self::state_key(id);

        // For simplicity, load current metadata and update fields
        let Some(current) = self.load_metadata(id).await? else {
            return Err(inngest_core::Error::Database("State not found".to_string()));
        };

        let mut updated = current;
        if let Some(rv) = update.request_version {
            updated.request_version = rv;
        }
        if let Some(started_at) = update.started_at {
            updated.started_at = started_at;
        }

        let context_json = serde_json::to_string(&updated.context).map_err(|e| {
            inngest_core::Error::InvalidData(format!("Failed to serialize context: {}", e))
        })?;

        sqlx::query(
            r#"
            UPDATE function_runs SET 
                request_version = ?, 
                started_at = ?,
                context = ?,
                updated_at = datetime('now')
            WHERE id = ?
            "#,
        )
        .bind(updated.request_version)
        .bind(updated.started_at.to_rfc3339())
        .bind(&context_json)
        .bind(&state_key)
        .execute(self.db.pool())
        .await
        .map_err(|e| inngest_core::Error::Database(format!("Failed to update metadata: {}", e)))?;

        Ok(())
    }

    async fn save_step(
        &self,
        id: &StateId,
        step_id: &str,
        data: serde_json::Value,
    ) -> Result<bool> {
        let state_key = Self::state_key(id);

        // Check if step already exists
        let existing =
            sqlx::query("SELECT step_data FROM function_steps WHERE run_id = ? AND step_id = ?")
                .bind(&state_key)
                .bind(step_id)
                .fetch_optional(self.db.pool())
                .await
                .map_err(|e| {
                    inngest_core::Error::Database(format!("Failed to check existing step: {}", e))
                })?;

        if let Some(row) = existing {
            let existing_data: serde_json::Value = serde_json::from_str(row.get("step_data"))
                .map_err(|e| {
                    inngest_core::Error::InvalidData(format!(
                        "Failed to parse existing step data: {}",
                        e
                    ))
                })?;

            if existing_data == data {
                return Ok(true); // Idempotent
            } else {
                return Err(inngest_core::Error::Database(
                    "Step data conflict".to_string(),
                ));
            }
        }

        // Get next step order
        let step_order: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(step_order), -1) + 1 FROM function_steps WHERE run_id = ?",
        )
        .bind(&state_key)
        .fetch_one(self.db.pool())
        .await
        .map_err(|e| inngest_core::Error::Database(format!("Failed to get step order: {}", e)))?;

        // Save new step
        let step_data_json = serde_json::to_string(&data).map_err(|e| {
            inngest_core::Error::InvalidData(format!("Failed to serialize step data: {}", e))
        })?;

        sqlx::query(
            "INSERT INTO function_steps (id, run_id, step_id, step_data, step_order) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(format!("{}:{}", state_key, step_id))
        .bind(&state_key)
        .bind(step_id)
        .bind(&step_data_json)
        .bind(step_order)
        .execute(self.db.pool())
        .await
        .map_err(|e| inngest_core::Error::Database(format!("Failed to save step: {}", e)))?;

        Ok(true)
    }

    async fn set_status(&self, id: &StateId, status: RunStatus) -> Result<()> {
        let state_key = Self::state_key(id);

        sqlx::query(
            "UPDATE function_runs SET status = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(status.to_string())
        .bind(&state_key)
        .execute(self.db.pool())
        .await
        .map_err(|e| inngest_core::Error::Database(format!("Failed to set status: {}", e)))?;

        Ok(())
    }

    async fn delete(&self, id: &StateId) -> Result<bool> {
        let state_key = Self::state_key(id);

        let result = sqlx::query("DELETE FROM function_runs WHERE id = ?")
            .bind(&state_key)
            .execute(self.db.pool())
            .await
            .map_err(|e| inngest_core::Error::Database(format!("Failed to delete state: {}", e)))?;

        Ok(result.rows_affected() > 0)
    }

    async fn exists(&self, id: &StateId) -> Result<bool> {
        let state_key = Self::state_key(id);

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM function_runs WHERE id = ?")
            .bind(&state_key)
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| {
                inngest_core::Error::Database(format!("Failed to check existence: {}", e))
            })?;

        Ok(count > 0)
    }
}

// Database row structures for export/import
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
pub struct SqliteRunRow {
    pub id: String,
    pub run_id: String,
    pub function_id: String,
    pub account_id: String,
    pub env_id: String,
    pub app_id: String,
    pub status: String,
    pub function_version: i64,
    pub event_id: String,
    pub event_ids: String,
    pub batch_id: Option<String>,
    pub original_run_id: Option<String>,
    pub started_at: String,
    pub debugger: i64,
    pub run_type: Option<String>,
    pub request_version: i64,
    pub context: String,
    pub span_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub priority_factor: Option<i64>,
    pub events: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
pub struct SqliteStepRow {
    pub id: String,
    pub run_id: String,
    pub step_id: String,
    pub step_data: String,
    pub step_order: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
pub struct SqliteStepInputRow {
    pub id: String,
    pub run_id: String,
    pub step_id: String,
    pub input_data: String,
    pub created_at: String,
}

/// SQLite data snapshot
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SqliteSnapshot {
    pub runs: Vec<SqliteRunRow>,
    pub steps: Vec<SqliteStepRow>,
    pub step_inputs: Vec<SqliteStepInputRow>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_sqlite_database_creation() {
        let db = SqliteDatabase::new_memory().await.unwrap();

        // Should be able to query the database
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM function_runs")
            .fetch_one(db.pool())
            .await
            .unwrap();

        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_sqlite_state_manager() {
        let db = Arc::new(SqliteDatabase::new_memory().await.unwrap());
        let manager = SqliteStateManager::new(db);

        let state_id = StateId {
            run_id: inngest_core::Ulid::new(),
            function_id: inngest_core::Uuid::new_v4(),
            tenant: inngest_state::Tenant {
                account_id: inngest_core::Uuid::new_v4(),
                env_id: inngest_core::Uuid::new_v4(),
                app_id: inngest_core::Uuid::new_v4(),
            },
        };

        let input = CreateStateInput {
            id: state_id.clone(),
            events: vec![serde_json::json!({"name": "test.event", "data": {}})],
            steps: vec![],
            step_inputs: vec![],
            metadata: Metadata::new(state_id.clone(), inngest_core::Ulid::new()),
        };

        // Create state
        let state = manager.create(input).await.unwrap();
        assert_eq!(state.metadata.id, state_id);

        // Load state
        let loaded = manager.load(&state_id).await.unwrap();
        assert!(loaded.is_some());

        // Check existence
        assert!(manager.exists(&state_id).await.unwrap());

        // Save step
        assert!(manager
            .save_step(&state_id, "step1", serde_json::json!({"output": "test"}))
            .await
            .unwrap());

        // Load updated state
        let updated = manager.load(&state_id).await.unwrap().unwrap();
        assert!(updated.step_completed("step1"));

        // Delete state
        assert!(manager.delete(&state_id).await.unwrap());
        assert!(!manager.exists(&state_id).await.unwrap());
    }
}
