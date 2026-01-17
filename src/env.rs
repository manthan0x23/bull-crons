use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Env {
    pub database_url: String,
    pub amqp_url: String,
}

impl Env {
    pub fn load() -> Result<Self> {
        // Load .env only if present (noop in prod)
        dotenvy::dotenv().ok();

        let database_url = std::env::var("DATABASE_URL")
            .context("DATABASE_URL is not set")?;

        let amqp_url = std::env::var("AMQP_URL")
            .context("AMQP_URL is not set")?;

        Ok(Self {
            database_url,
            amqp_url,
        })
    }
}
