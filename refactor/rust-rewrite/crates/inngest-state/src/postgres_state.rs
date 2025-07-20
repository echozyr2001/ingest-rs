use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;

use crate::{RunMetadata, RunState, StateError, StateManager, StepState};
use inngest_core::{RunStatus, StateId};

#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub max_connections: usize,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            database: "inngest".to_string(),
            username: "postgres".to_string(),
            password: "password".to_string(),
            max_connections: 10,
        }
    }
}

pub struct PostgresStateManager {
    pool: Pool<Postgres>,
}

impl PostgresStateManager {
    pub async fn new(config: PostgresConfig) -> Result<Self, StateError> {
        let database_url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            config.username, config.password, config.host, config.port, config.database
        );

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.max_connections as u32)
            .connect(&database_url)
            .await
            .map_err(|e| StateError::Database(e))?;

        let manager = Self { pool };
        manager.init_schema().await?;
        Ok(manager)
    }

    pub async fn from_url(url: &str) -> Result<Self, StateError> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await
            .map_err(|e| StateError::Database(e))?;

        let manager = Self { pool };
        manager.init_schema().await?;
        Ok(manager)
    }

    async fn init_schema(&self) -> Result<(), StateError> {
        // Create function_runs table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS function_runs (
                id UUID PRIMARY KEY,
                function_id UUID NOT NULL,
                function_version INTEGER NOT NULL,
                status TEXT NOT NULL,
                started_at TIMESTAMPTZ,
                ended_at TIMESTAMPTZ,
                event_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
                idempotency_key TEXT,
                batch_id UUID,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StateError::Database(e))?;

        // Create function_run_steps table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS function_run_steps (
                run_id UUID NOT NULL REFERENCES function_runs(id) ON DELETE CASCADE,
                step_id TEXT NOT NULL,
                status TEXT NOT NULL,
                input_data JSONB,
                output_data JSONB,
                error_message TEXT,
                attempt INTEGER NOT NULL DEFAULT 1,
                started_at TIMESTAMPTZ,
                completed_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                PRIMARY KEY (run_id, step_id)
            );
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StateError::Database(e))?;

        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_function_runs_function_id ON function_runs(function_id);")
            .execute(&self.pool)
            .await
            .map_err(|e| StateError::Database(e))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_function_runs_status ON function_runs(status);",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StateError::Database(e))?;

        Ok(())
    }
}

#[async_trait]
impl StateManager for PostgresStateManager {
    async fn create_run(&self, metadata: &RunMetadata) -> Result<(), StateError> {
        sqlx::query(
            r#"
            INSERT INTO function_runs (id, function_id, function_version, status, started_at, event_ids, idempotency_key, batch_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(metadata.id.as_uuid())
        .bind(metadata.function_id)
        .bind(metadata.function_version as i32)
        .bind(serde_json::to_string(&metadata.status)?)
        .bind(metadata.started_at)
        .bind(serde_json::to_value(&metadata.event_ids)?)
        .bind(&metadata.idempotency_key)
        .bind(metadata.batch_id)
        .execute(&self.pool)
        .await
        .map_err(|e| StateError::Database(e))?;

        Ok(())
    }

    async fn load_run(&self, id: &StateId) -> Result<RunState, StateError> {
        // Load run metadata
        let run_row = sqlx::query(
            r#"
            SELECT id, function_id, function_version, status, started_at, ended_at, 
                   event_ids, idempotency_key, batch_id
            FROM function_runs WHERE id = $1
            "#,
        )
        .bind(id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StateError::Database(e))?;

        let run_row = run_row.ok_or_else(|| StateError::RunNotFound { id: id.clone() })?;

        let metadata = RunMetadata {
            id: id.clone(),
            function_id: run_row.get("function_id"),
            function_version: run_row.get::<i32, _>("function_version") as u32,
            status: serde_json::from_str(&run_row.get::<String, _>("status"))?,
            started_at: run_row.get("started_at"),
            ended_at: run_row.get("ended_at"),
            event_ids: serde_json::from_value(run_row.get("event_ids"))?,
            idempotency_key: run_row.get("idempotency_key"),
            batch_id: run_row.get("batch_id"),
        };

        // Load steps
        let step_rows = sqlx::query(
            r#"
            SELECT step_id, status, input_data, output_data, error_message, attempt, started_at, completed_at
            FROM function_run_steps WHERE run_id = $1 ORDER BY created_at
            "#,
        )
        .bind(id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StateError::Database(e))?;

        let steps = step_rows
            .into_iter()
            .map(|row| -> Result<StepState, StateError> {
                Ok(StepState {
                    id: row.get("step_id"),
                    status: serde_json::from_str(&row.get::<String, _>("status"))?,
                    input: row.get("input_data"),
                    output: row.get("output_data"),
                    error: row.get("error_message"),
                    attempt: row.get::<i32, _>("attempt") as u32,
                    started_at: row.get("started_at"),
                    completed_at: row.get("completed_at"),
                })
            })
            .collect::<Result<Vec<_>, StateError>>()?;

        Ok(RunState {
            metadata,
            steps,
            stack: Vec::new(),
            ctx: None,
        })
    }

    async fn update_run_metadata(&self, metadata: &RunMetadata) -> Result<(), StateError> {
        sqlx::query(
            r#"
            UPDATE function_runs 
            SET function_id = $2, function_version = $3, status = $4, started_at = $5, 
                ended_at = $6, event_ids = $7, idempotency_key = $8, batch_id = $9,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(metadata.id.as_uuid())
        .bind(metadata.function_id)
        .bind(metadata.function_version as i32)
        .bind(serde_json::to_string(&metadata.status)?)
        .bind(metadata.started_at)
        .bind(metadata.ended_at)
        .bind(serde_json::to_value(&metadata.event_ids)?)
        .bind(&metadata.idempotency_key)
        .bind(metadata.batch_id)
        .execute(&self.pool)
        .await
        .map_err(|e| StateError::Database(e))?;

        Ok(())
    }

    async fn update_run_status(&self, id: &StateId, status: RunStatus) -> Result<(), StateError> {
        let ended_at = match status {
            RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled => Some(Utc::now()),
            _ => None,
        };

        sqlx::query(
            r#"
            UPDATE function_runs 
            SET status = $2, ended_at = $3, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id.as_uuid())
        .bind(serde_json::to_string(&status)?)
        .bind(ended_at)
        .execute(&self.pool)
        .await
        .map_err(|e| StateError::Database(e))?;

        Ok(())
    }

    async fn save_step(&self, run_id: &StateId, step: &StepState) -> Result<(), StateError> {
        sqlx::query(
            r#"
            INSERT INTO function_run_steps (run_id, step_id, status, input_data, output_data, error_message, attempt, started_at, completed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (run_id, step_id) DO UPDATE SET
                status = EXCLUDED.status,
                input_data = EXCLUDED.input_data,
                output_data = EXCLUDED.output_data,
                error_message = EXCLUDED.error_message,
                attempt = EXCLUDED.attempt,
                started_at = EXCLUDED.started_at,
                completed_at = EXCLUDED.completed_at,
                updated_at = NOW()
            "#,
        )
        .bind(run_id.as_uuid())
        .bind(&step.id)
        .bind(serde_json::to_string(&step.status)?)
        .bind(&step.input)
        .bind(&step.output)
        .bind(&step.error)
        .bind(step.attempt as i32)
        .bind(step.started_at)
        .bind(step.completed_at)
        .execute(&self.pool)
        .await
        .map_err(|e| StateError::Database(e))?;

        Ok(())
    }

    async fn load_step(&self, run_id: &StateId, step_id: &str) -> Result<StepState, StateError> {
        let row = sqlx::query(
            r#"
            SELECT step_id, status, input_data, output_data, error_message, attempt, started_at, completed_at
            FROM function_run_steps WHERE run_id = $1 AND step_id = $2
            "#,
        )
        .bind(run_id.as_uuid())
        .bind(step_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StateError::Database(e))?;

        let row = row.ok_or_else(|| StateError::StepNotFound {
            step_id: step_id.to_string(),
            run_id: run_id.clone(),
        })?;

        Ok(StepState {
            id: row.get("step_id"),
            status: serde_json::from_str(&row.get::<String, _>("status"))?,
            input: row.get("input_data"),
            output: row.get("output_data"),
            error: row.get("error_message"),
            attempt: row.get::<i32, _>("attempt") as u32,
            started_at: row.get("started_at"),
            completed_at: row.get("completed_at"),
        })
    }

    async fn load_events(&self, id: &StateId) -> Result<Vec<Value>, StateError> {
        let row = sqlx::query("SELECT event_ids FROM function_runs WHERE id = $1")
            .bind(id.as_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StateError::Database(e))?;

        if let Some(row) = row {
            let event_ids: Vec<Uuid> = serde_json::from_value(row.get("event_ids"))?;

            // Convert UUIDs to JSON values for now
            Ok(event_ids
                .into_iter()
                .map(|id| serde_json::json!({"id": id}))
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    async fn save_events(&self, id: &StateId, events: &[Value]) -> Result<(), StateError> {
        let event_ids: Vec<Uuid> = events
            .iter()
            .filter_map(|event| {
                event
                    .get("id")
                    .and_then(|id| id.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
            })
            .collect();

        sqlx::query("UPDATE function_runs SET event_ids = $2, updated_at = NOW() WHERE id = $1")
            .bind(id.as_uuid())
            .bind(serde_json::to_value(&event_ids)?)
            .execute(&self.pool)
            .await
            .map_err(|e| StateError::Database(e))?;

        Ok(())
    }

    async fn delete_run(&self, id: &StateId) -> Result<(), StateError> {
        // Delete steps first (foreign key constraint)
        sqlx::query("DELETE FROM function_run_steps WHERE run_id = $1")
            .bind(id.as_uuid())
            .execute(&self.pool)
            .await
            .map_err(|e| StateError::Database(e))?;

        // Delete run
        sqlx::query("DELETE FROM function_runs WHERE id = $1")
            .bind(id.as_uuid())
            .execute(&self.pool)
            .await
            .map_err(|e| StateError::Database(e))?;

        Ok(())
    }

    async fn run_exists(&self, id: &StateId) -> Result<bool, StateError> {
        let result = sqlx::query("SELECT 1 FROM function_runs WHERE id = $1")
            .bind(id.as_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StateError::Database(e))?;

        Ok(result.is_some())
    }

    async fn list_runs_by_function(
        &self,
        function_id: Uuid,
    ) -> Result<Vec<RunMetadata>, StateError> {
        let rows = sqlx::query(
            r#"
            SELECT id, function_id, function_version, status, started_at, ended_at, 
                   event_ids, idempotency_key, batch_id
            FROM function_runs WHERE function_id = $1 ORDER BY started_at DESC
            "#,
        )
        .bind(function_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StateError::Database(e))?;

        let mut runs = Vec::new();
        for row in rows {
            runs.push(RunMetadata {
                id: StateId::from_uuid(row.get("id")),
                function_id: row.get("function_id"),
                function_version: row.get::<i32, _>("function_version") as u32,
                status: serde_json::from_str(&row.get::<String, _>("status"))?,
                started_at: row.get("started_at"),
                ended_at: row.get("ended_at"),
                event_ids: serde_json::from_value(row.get("event_ids"))?,
                idempotency_key: row.get("idempotency_key"),
                batch_id: row.get("batch_id"),
            });
        }

        Ok(runs)
    }

    async fn list_active_runs(&self) -> Result<Vec<RunMetadata>, StateError> {
        let rows = sqlx::query(
            r#"
            SELECT id, function_id, function_version, status, started_at, ended_at, 
                   event_ids, idempotency_key, batch_id
            FROM function_runs 
            WHERE status IN ('Queued', 'Running', 'Paused')
            ORDER BY started_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StateError::Database(e))?;

        let mut runs = Vec::new();
        for row in rows {
            runs.push(RunMetadata {
                id: StateId::from_uuid(row.get("id")),
                function_id: row.get("function_id"),
                function_version: row.get::<i32, _>("function_version") as u32,
                status: serde_json::from_str(&row.get::<String, _>("status"))?,
                started_at: row.get("started_at"),
                ended_at: row.get("ended_at"),
                event_ids: serde_json::from_value(row.get("event_ids"))?,
                idempotency_key: row.get("idempotency_key"),
                batch_id: row.get("batch_id"),
            });
        }

        Ok(runs)
    }
}
