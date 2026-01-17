## Setup Instructions

1. **Install Dependencies**:
   - PostgreSQL
   - RabbitMQ

2. **Set Environment Variables**:
   ```bash
   export DATABASE_URL="postgres://user:pass@localhost/webhooks"
   export AMQP_URL="amqp://guest:guest@localhost:5672"
   ```

3. **Run**:
   ```bash
   cargo run
   ```

## Features

- ✅ RabbitMQ-based job queuing
- ✅ PostgreSQL for persistence and tracking
- ✅ Exponential backoff retry strategy
- ✅ Configurable max retries
- ✅ Webhook result tracking
- ✅ Support for multiple HTTP methods
- ✅ Custom headers and body
- ✅ Graceful failure handling
- ✅ Job status tracking