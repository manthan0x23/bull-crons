use crate::models::WebhookJob;
use anyhow::{Context, Result};
use lapin::{
    options::*, types::FieldTable, Channel, Connection, ConnectionProperties, Consumer,
};
use tracing::info;

const QUEUE_NAME: &str = "webhook_jobs";
const RETRY_QUEUE_NAME: &str = "webhook_jobs_retry";

pub struct RabbitMQClient {
    channel: Channel,
}

impl RabbitMQClient {
    pub async fn connect(amqp_url: &str) -> Result<Self> {
        let conn = Connection::connect(amqp_url, ConnectionProperties::default())
            .await
            .context("Failed to connect to RabbitMQ")?;

        let channel = conn.create_channel().await?;

        // Declare main queue
        channel
            .queue_declare(
                QUEUE_NAME,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        // Declare retry queue with message TTL
        let mut retry_args = FieldTable::default();
        use lapin::types::AMQPValue;
        use lapin::types::LongString;
        retry_args.insert(
            "x-dead-letter-exchange".into(),
            AMQPValue::LongString(LongString::from("")),
        );
        retry_args.insert(
            "x-dead-letter-routing-key".into(),
            AMQPValue::LongString(LongString::from(QUEUE_NAME)),
        );

        channel
            .queue_declare(
                RETRY_QUEUE_NAME,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                retry_args,
            )
            .await?;

        info!("Connected to RabbitMQ and queues declared");

        Ok(Self { channel })
    }

    pub async fn publish_job(&self, job: &WebhookJob) -> Result<()> {
        let payload = serde_json::to_vec(job)?;

        let delay_ms = if job.retry_count > 0 {
            job.calculate_retry_delay() * 1000
        } else {
            0
        };

        let queue = if delay_ms > 0 {
            let props = lapin::BasicProperties::default()
                .with_delivery_mode(2)
                .with_expiration(delay_ms.to_string().into());

            self.channel
                .basic_publish(
                    "",
                    RETRY_QUEUE_NAME,
                    BasicPublishOptions::default(),
                    &payload,
                    props,
                )
                .await?;

            RETRY_QUEUE_NAME
        } else {
            self.channel
                .basic_publish(
                    "",
                    QUEUE_NAME,
                    BasicPublishOptions::default(),
                    &payload,
                    lapin::BasicProperties::default().with_delivery_mode(2),
                )
                .await?;

            QUEUE_NAME
        };

        info!("Published job {} to queue {}", job.id, queue);
        Ok(())
    }

    pub async fn consume(&self) -> Result<Consumer> {
        let consumer = self
            .channel
            .basic_consume(
                QUEUE_NAME,
                "webhook_worker",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        Ok(consumer)
    }

    pub async fn ack(&self, delivery_tag: u64) -> Result<()> {
        self.channel
            .basic_ack(delivery_tag, BasicAckOptions::default())
            .await?;
        Ok(())
    }

    pub async fn nack(&self, delivery_tag: u64) -> Result<()> {
        self.channel
            .basic_nack(delivery_tag, BasicNackOptions::default())
            .await?;
        Ok(())
    }
}