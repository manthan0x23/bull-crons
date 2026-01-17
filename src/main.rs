mod env;
mod executor;
mod models;
mod queue;
mod storage;
mod worker;

use anyhow::Result;
use std::sync::Arc;
use tracing::info;

use env::Env;
use models::{HttpMethod, WebhookJob};
use queue::RabbitMQClient;
use storage::{PostgresStorage, Storage};
use worker::Worker;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Load config
    let env = Env::load()?;

    // Storage
    let pg_storage = PostgresStorage::new(&env.database_url).await?;
    pg_storage.init_schema().await?;
    let storage: Arc<dyn Storage> = Arc::new(pg_storage);

    // Queue
    let queue = Arc::new(RabbitMQClient::connect(&env.amqp_url).await?);

    // Test job
    let job = WebhookJob::new(
        "https://httpbin.org/post".to_string(),
        HttpMethod::POST,
        vec![("Content-Type".to_string(), "application/json".to_string())],
        Some(r#"{"test": "data"}"#.to_string()),
        3,
    );

    storage.save_job(&job).await?;
    queue.publish_job(&job).await?;
    info!("Published test job {}", job.id);

    let worker = Worker::new(queue.clone(), storage.clone());
    worker.run().await?;

    Ok(())
}
