use crate::models::{HttpMethod, JobStatus, WebhookJob, WebhookResult};
use crate::storage::Storage;
use anyhow::Result;
use chrono::Utc;
use reqwest::Client;
use std::sync::Arc;
use tracing::{error, info};

pub struct WebhookExecutor {
    client: Client,
    storage: Arc<dyn Storage>,
}

impl WebhookExecutor {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
            storage,
        }
    }

    pub async fn execute(&self, mut job: WebhookJob) -> Result<bool> {
        info!("Executing webhook job {} to {}", job.id, job.url);

        job.status = JobStatus::Processing;
        self.storage.update_job_status(job.id, JobStatus::Processing).await?;

        let result = self.send_request(&job).await;

        match result {
            Ok(webhook_result) => {
                if let Some(status) = webhook_result.status_code {
                    if (200..300).contains(&status) {
                        info!("Job {} succeeded with status {}", job.id, status);
                        job.status = JobStatus::Success;
                        self.storage.update_job_status(job.id, JobStatus::Success).await?;
                        self.storage.save_result(&webhook_result).await?;
                        return Ok(true);
                    }
                }

                error!("Job {} failed: {:?}", job.id, webhook_result.error);
                self.storage.save_result(&webhook_result).await?;
                Ok(false)
            }
            Err(e) => {
                error!("Job {} error: {}", job.id, e);
                let webhook_result = WebhookResult {
                    job_id: job.id,
                    status_code: None,
                    response_body: None,
                    error: Some(e.to_string()),
                    attempted_at: Utc::now(),
                };
                self.storage.save_result(&webhook_result).await?;
                Ok(false)
            }
        }
    }

    async fn send_request(&self, job: &WebhookJob) -> Result<WebhookResult> {
        let mut request = match job.method {
            HttpMethod::GET => self.client.get(&job.url),
            HttpMethod::POST => self.client.post(&job.url),
            HttpMethod::PUT => self.client.put(&job.url),
            HttpMethod::PATCH => self.client.patch(&job.url),
            HttpMethod::DELETE => self.client.delete(&job.url),
        };

        for (key, value) in &job.headers {
            request = request.header(key, value);
        }

        if let Some(body) = &job.body {
            request = request.body(body.clone());
        }

        let start = Utc::now();
        let response = request.send().await?;
        let status_code = response.status().as_u16();
        let response_body = response.text().await.ok();

        Ok(WebhookResult {
            job_id: job.id,
            status_code: Some(status_code),
            response_body,
            error: None,
            attempted_at: start,
        })
    }
}