mod executor;
mod models;
mod queue;
mod storage;
mod worker;

use anyhow::Result;
use models::{HttpMethod, WebhookJob};
use queue::RabbitMQClient;
use storage::{PostgresStorage, Storage};
use std::sync::Arc;
use tracing::info;
use worker::Worker;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Configuration
    let amqp_url = std::env::var("AMQP_URL")
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".to_string());
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/webhooks".to_string());

    // Initialize storage
    let pg_storage = PostgresStorage::new(&database_url).await?;
    pg_storage.init_schema().await?;
    let storage: Arc<dyn Storage> = Arc::new(pg_storage);

    // Initialize queue
    let queue = Arc::new(RabbitMQClient::connect(&amqp_url).await?);

    // Example: Create and publish a job
    let job = WebhookJob::new(
        "https://httpbin.org/post".to_string(),
        HttpMethod::POST,
        vec![("Content-Type".to_string(), "application/json".to_string())],
        Some(r#"{"test": "data"}"#.to_string()),
        3, // max retries
    );

    storage.save_job(&job).await?;
    queue.publish_job(&job).await?;
    info!("Published test job {}", job.id);

    // Start worker
    let worker = Worker::new(queue.clone(), storage.clone());
    worker.run().await?;

    Ok(())
}