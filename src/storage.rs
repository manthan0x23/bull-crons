use crate::models::{JobStatus, WebhookJob, WebhookResult};
use anyhow::Result;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[async_trait]
pub trait Storage: Send + Sync {
    async fn save_job(&self, job: &WebhookJob) -> Result<()>;
    async fn update_job_status(&self, job_id: Uuid, status: JobStatus) -> Result<()>;
    async fn save_result(&self, result: &WebhookResult) -> Result<()>;
    async fn get_job(&self, job_id: Uuid) -> Result<Option<WebhookJob>>;
    async fn get_job_results(&self, job_id: Uuid) -> Result<Vec<WebhookResult>>;
}

pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn init_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS webhook_jobs (
                id UUID PRIMARY KEY,
                url TEXT NOT NULL,
                method TEXT NOT NULL,
                headers JSONB NOT NULL,
                body TEXT,
                retry_count INTEGER NOT NULL,
                max_retries INTEGER NOT NULL,
                created_at TIMESTAMPTZ NOT NULL,
                scheduled_at TIMESTAMPTZ NOT NULL,
                status TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS webhook_results (
                id SERIAL PRIMARY KEY,
                job_id UUID NOT NULL REFERENCES webhook_jobs(id),
                status_code INTEGER,
                response_body TEXT,
                error TEXT,
                attempted_at TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[async_trait]
impl Storage for PostgresStorage {
    async fn save_job(&self, job: &WebhookJob) -> Result<()> {
        let headers_json = serde_json::to_value(&job.headers)?;
        let method = format!("{:?}", job.method);
        let status = format!("{:?}", job.status);

        sqlx::query(
            r#"
            INSERT INTO webhook_jobs 
            (id, url, method, headers, body, retry_count, max_retries, created_at, scheduled_at, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id) DO UPDATE SET
                retry_count = $6,
                scheduled_at = $9,
                status = $10
            "#,
        )
        .bind(job.id)
        .bind(&job.url)
        .bind(method)
        .bind(headers_json)
        .bind(&job.body)
        .bind(job.retry_count as i32)
        .bind(job.max_retries as i32)
        .bind(job.created_at)
        .bind(job.scheduled_at)
        .bind(status)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_job_status(&self, job_id: Uuid, status: JobStatus) -> Result<()> {
        let status_str = format!("{:?}", status);
        sqlx::query("UPDATE webhook_jobs SET status = $1 WHERE id = $2")
            .bind(status_str)
            .bind(job_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn save_result(&self, result: &WebhookResult) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO webhook_results 
            (job_id, status_code, response_body, error, attempted_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(result.job_id)
        .bind(result.status_code.map(|c| c as i32))
        .bind(&result.response_body)
        .bind(&result.error)
        .bind(result.attempted_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_job(&self, job_id: Uuid) -> Result<Option<WebhookJob>> {
        let row = sqlx::query("SELECT * FROM webhook_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let headers: Vec<(String, String)> = serde_json::from_value(row.get("headers"))?;
            let method = match row.get::<String, _>("method").as_str() {
                "GET" => crate::models::HttpMethod::GET,
                "POST" => crate::models::HttpMethod::POST,
                "PUT" => crate::models::HttpMethod::PUT,
                "PATCH" => crate::models::HttpMethod::PATCH,
                "DELETE" => crate::models::HttpMethod::DELETE,
                _ => crate::models::HttpMethod::POST,
            };

            Ok(Some(WebhookJob {
                id: row.get("id"),
                url: row.get("url"),
                method,
                headers,
                body: row.get("body"),
                retry_count: row.get::<i32, _>("retry_count") as u32,
                max_retries: row.get::<i32, _>("max_retries") as u32,
                created_at: row.get("created_at"),
                scheduled_at: row.get("scheduled_at"),
                status: JobStatus::Pending, // Simplified for this example
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_job_results(&self, job_id: Uuid) -> Result<Vec<WebhookResult>> {
        let rows = sqlx::query("SELECT * FROM webhook_results WHERE job_id = $1 ORDER BY attempted_at")
            .bind(job_id)
            .fetch_all(&self.pool)
            .await?;

        let results = rows
            .into_iter()
            .map(|row| WebhookResult {
                job_id: row.get("job_id"),
                status_code: row.get::<Option<i32>, _>("status_code").map(|c| c as u16),
                response_body: row.get("response_body"),
                error: row.get("error"),
                attempted_at: row.get("attempted_at"),
            })
            .collect();

        Ok(results)
    }
}