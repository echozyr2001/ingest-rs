use anyhow::Result;
use async_trait::async_trait;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;

use inngest_core::{
    function::{Function, FunctionConfig},
    run::Run,
};

/// Extended state manager for production that includes function management
#[async_trait]
pub trait ProductionStateManager: Send + Sync {
    /// Register a function in the database
    async fn register_function(&self, function: &Function) -> Result<bool>;

    /// List all registered functions
    async fn list_functions(&self) -> Result<Vec<Function>>;

    /// Get functions that should trigger for a given event
    async fn get_functions_for_event(&self, event_name: &str) -> Result<Vec<Function>>;

    /// Get a specific function by ID
    async fn get_function(&self, function_id: &str) -> Result<Option<Function>>;

    /// Unregister a function
    async fn unregister_function(&self, function_id: &str) -> Result<bool>;

    /// Create a function run record
    async fn create_run(&self, run: &Run) -> Result<()>;

    /// Get a specific run by ID
    async fn get_run(&self, run_id: &str) -> Result<Option<Run>>;

    /// List runs with optional filtering
    async fn list_runs(
        &self,
        limit: usize,
        offset: usize,
        status: Option<String>,
        function_id: Option<String>,
    ) -> Result<Vec<Run>>;

    /// Update run status
    async fn update_run_status(&self, run_id: &str, status: &str) -> Result<()>;

    /// Health check for database connection
    async fn health_check(&self) -> Result<()>;
}

/// PostgreSQL implementation of ProductionStateManager
#[derive(Clone)]
pub struct PostgresProductionStateManager {
    pool: Pool<Postgres>,
}

impl PostgresProductionStateManager {
    pub async fn from_url(database_url: &str) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;

        let manager = Self { pool };
        manager.initialize_schema().await?;
        Ok(manager)
    }

    async fn initialize_schema(&self) -> Result<()> {
        // Create functions table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS functions (
                id VARCHAR PRIMARY KEY,
                name VARCHAR NOT NULL,
                slug VARCHAR,
                config JSONB NOT NULL,
                triggers JSONB NOT NULL,
                steps JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create function_runs table (extend the existing schema)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS function_runs (
                id VARCHAR PRIMARY KEY,
                function_id VARCHAR NOT NULL,
                event_id VARCHAR,
                status VARCHAR NOT NULL,
                started_at TIMESTAMPTZ,
                ended_at TIMESTAMPTZ,
                output JSONB,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create function_run_steps table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS function_run_steps (
                id VARCHAR,
                run_id VARCHAR NOT NULL,
                name VARCHAR NOT NULL,
                status VARCHAR NOT NULL,
                input JSONB,
                output JSONB,
                error_message TEXT,
                started_at TIMESTAMPTZ,
                ended_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                PRIMARY KEY (run_id, id)
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_functions_name ON functions(name);")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_function_runs_function_id ON function_runs(function_id);")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_function_runs_status ON function_runs(status);",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[async_trait]
impl ProductionStateManager for PostgresProductionStateManager {
    async fn register_function(&self, function: &Function) -> Result<bool> {
        let config_json = serde_json::to_value(&function.config)?;
        let triggers_json = serde_json::to_value(&function.config.triggers)?;
        let steps_json = serde_json::to_value(&function.config.steps)?;

        let result = sqlx::query(
            r#"
            INSERT INTO functions (id, name, slug, config, triggers, steps)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                slug = EXCLUDED.slug,
                config = EXCLUDED.config,
                triggers = EXCLUDED.triggers,
                steps = EXCLUDED.steps,
                updated_at = NOW()
            "#,
        )
        .bind(&function.config.id)
        .bind(&function.config.name)
        .bind(&function.slug)
        .bind(config_json)
        .bind(triggers_json)
        .bind(steps_json)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn list_functions(&self) -> Result<Vec<Function>> {
        let rows = sqlx::query(
            "SELECT id, name, slug, config, triggers, steps FROM functions ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut functions = Vec::new();
        for row in rows {
            let config: FunctionConfig = serde_json::from_value(row.get("config"))?;

            // Generate a UUID for the Function.id field, since it expects UUID
            // The actual function identifier is in config.id
            functions.push(Function {
                id: Uuid::new_v4(), // Generate new UUID for Function.id
                config,
                version: 1,             // Default version
                app_id: Uuid::new_v4(), // Generate default app_id
                slug: row.get::<Option<String>, _>("slug").unwrap_or_default(),
            });
        }

        Ok(functions)
    }

    async fn get_functions_for_event(&self, event_name: &str) -> Result<Vec<Function>> {
        // Query functions that have triggers matching this event
        let rows = sqlx::query(
            r#"
            SELECT id, name, slug, config, triggers, steps 
            FROM functions 
            WHERE triggers::text ILIKE '%' || $1 || '%'
            "#,
        )
        .bind(event_name)
        .fetch_all(&self.pool)
        .await?;

        let mut functions = Vec::new();
        for row in rows {
            let config: FunctionConfig = serde_json::from_value(row.get("config"))?;

            // More sophisticated trigger matching could be added here
            // For now, we do a simple string match in the triggers JSON
            let function = Function {
                id: Uuid::new_v4(), // Generate new UUID for Function.id
                config,
                version: 1,
                app_id: Uuid::new_v4(),
                slug: row.get::<Option<String>, _>("slug").unwrap_or_default(),
            };

            // TODO: Implement proper trigger matching logic
            // For now, include all functions that mention the event name
            functions.push(function);
        }

        Ok(functions)
    }

    async fn get_function(&self, function_id: &str) -> Result<Option<Function>> {
        let row = sqlx::query(
            "SELECT id, name, slug, config, triggers, steps FROM functions WHERE id = $1",
        )
        .bind(function_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let config: FunctionConfig = serde_json::from_value(row.get("config"))?;

            Ok(Some(Function {
                id: Uuid::new_v4(), // Generate new UUID for Function.id
                config,
                version: 1,
                app_id: Uuid::new_v4(),
                slug: row.get::<Option<String>, _>("slug").unwrap_or_default(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn unregister_function(&self, function_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM functions WHERE id = $1")
            .bind(function_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn create_run(&self, run: &Run) -> Result<()> {
        let output_json = run
            .output
            .as_ref()
            .map(|o| serde_json::to_value(o))
            .transpose()?;

        sqlx::query(
            r#"
            INSERT INTO function_runs (id, function_id, event_id, status, started_at, ended_at, output)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&run.id)
        .bind(&run.function_id)
        .bind(&run.event_id)
        .bind(&run.status)
        .bind(run.started_at)
        .bind(run.ended_at)
        .bind(output_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_run(&self, run_id: &str) -> Result<Option<Run>> {
        let row = sqlx::query(
            r#"
            SELECT id, function_id, event_id, status, started_at, ended_at, output
            FROM function_runs 
            WHERE id = $1
            "#,
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let output: Option<serde_json::Value> = row.get("output");

            Ok(Some(Run {
                id: row.get("id"),
                function_id: row.get("function_id"),
                event_id: row.get("event_id"),
                status: row.get("status"),
                started_at: row.get("started_at"),
                ended_at: row.get("ended_at"),
                output,
                steps: vec![], // Steps would need to be loaded separately if needed
            }))
        } else {
            Ok(None)
        }
    }

    async fn list_runs(
        &self,
        limit: usize,
        offset: usize,
        status: Option<String>,
        function_id: Option<String>,
    ) -> Result<Vec<Run>> {
        let mut conditions = Vec::new();
        let mut param_count = 0;

        if status.is_some() {
            param_count += 1;
            conditions.push(format!("status = ${}", param_count));
        }
        if function_id.is_some() {
            param_count += 1;
            conditions.push(format!("function_id = ${}", param_count));
        }

        let mut query = "SELECT id, function_id, event_id, status, started_at, ended_at, output FROM function_runs".to_string();

        if !conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(" ORDER BY created_at DESC");
        query.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

        let mut query_builder = sqlx::query(&query);

        if let Some(status) = &status {
            query_builder = query_builder.bind(status);
        }
        if let Some(function_id) = &function_id {
            query_builder = query_builder.bind(function_id);
        }

        let rows = query_builder.fetch_all(&self.pool).await?;

        let mut runs = Vec::new();
        for row in rows {
            let output: Option<serde_json::Value> = row.get("output");

            runs.push(Run {
                id: row.get("id"),
                function_id: row.get("function_id"),
                event_id: row.get("event_id"),
                status: row.get("status"),
                started_at: row.get("started_at"),
                ended_at: row.get("ended_at"),
                output,
                steps: vec![], // Steps would need to be loaded separately if needed
            });
        }

        Ok(runs)
    }
    async fn update_run_status(&self, run_id: &str, status: &str) -> Result<()> {
        sqlx::query("UPDATE function_runs SET status = $1, updated_at = NOW() WHERE id = $2")
            .bind(status)
            .bind(run_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }
}
