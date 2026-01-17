use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookJob {
    pub id: Uuid,
    pub url: String,
    pub method: HttpMethod,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub created_at: DateTime<Utc>,
    pub scheduled_at: DateTime<Utc>,
    pub status: JobStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    Pending,
    Processing,
    Success,
    Failed,
    RetryScheduled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookResult {
    pub job_id: Uuid,
    pub status_code: Option<u16>,
    pub response_body: Option<String>,
    pub error: Option<String>,
    pub attempted_at: DateTime<Utc>,
}

impl WebhookJob {
    pub fn new(
        url: String,
        method: HttpMethod,
        headers: Vec<(String, String)>,
        body: Option<String>,
        max_retries: u32,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            url,
            method,
            headers,
            body,
            retry_count: 0,
            max_retries,
            created_at: now,
            scheduled_at: now,
            status: JobStatus::Pending,
        }
    }

    pub fn calculate_retry_delay(&self) -> i64 {
        // Exponential backoff: 2^retry_count seconds (1s, 2s, 4s, 8s, ...)
        let base: i64 = 2;
        base.pow(self.retry_count)
    }

    pub fn schedule_retry(&mut self) {
        self.retry_count += 1;
        self.status = JobStatus::RetryScheduled;
        let delay = self.calculate_retry_delay();
        self.scheduled_at = Utc::now() + chrono::Duration::seconds(delay);
    }
}