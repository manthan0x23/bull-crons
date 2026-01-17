use crate::executor::WebhookExecutor;
use crate::models::{JobStatus, WebhookJob};
use crate::queue::RabbitMQClient;
use crate::storage::Storage;
use anyhow::Result;
use futures_lite::StreamExt;
use std::sync::Arc;
use tracing::{error, info};

pub struct Worker {
    queue: Arc<RabbitMQClient>,
    executor: WebhookExecutor,
    storage: Arc<dyn Storage>,
}

impl Worker {
    pub fn new(
        queue: Arc<RabbitMQClient>,
        storage: Arc<dyn Storage>,
    ) -> Self {
        let executor = WebhookExecutor::new(storage.clone());
        Self {
            queue,
            executor,
            storage,
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Worker starting...");
        let mut consumer = self.queue.consume().await?;

        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    let job: WebhookJob = match serde_json::from_slice(&delivery.data) {
                        Ok(j) => j,
                        Err(e) => {
                            error!("Failed to deserialize job: {}", e);
                            self.queue.nack(delivery.delivery_tag).await?;
                            continue;
                        }
                    };

                    info!("Processing job {}", job.id);

                    let success = match self.executor.execute(job.clone()).await {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Job execution error: {}", e);
                            false
                        }
                    };

                    if success {
                        self.queue.ack(delivery.delivery_tag).await?;
                    } else {
                        // Handle retry logic
                        if job.retry_count < job.max_retries {
                            let mut retry_job = job.clone();
                            retry_job.schedule_retry();

                            info!(
                                "Scheduling retry {} for job {}",
                                retry_job.retry_count, retry_job.id
                            );

                            self.storage.save_job(&retry_job).await?;
                            self.queue.publish_job(&retry_job).await?;
                            self.queue.ack(delivery.delivery_tag).await?;
                        } else {
                            error!("Job {} exhausted all retries", job.id);
                            self.storage
                                .update_job_status(job.id, JobStatus::Failed)
                                .await?;
                            self.queue.ack(delivery.delivery_tag).await?;
                        }
                    }
                }
                Err(e) => {
                    error!("Consumer error: {}", e);
                }
            }
        }

        Ok(())
    }
}