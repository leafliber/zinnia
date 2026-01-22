# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Zinnia is a high-performance time-series backend service for device battery monitoring and alerting systems, built with Rust.

### Tech Stack
- **Language**: Rust (edition 2021)
- **Web Framework**: Actix Web 4.x
- **Database**: TimescaleDB (PostgreSQL extension) with SQLx ORM
- **Cache/Queue**: Redis 7+
- **Authentication**: JWT + API Key (dual-token architecture)
- **Messaging**: WebSocket support for real-time communication
- **Notifications**: Web Push with VAPID, Email via SMTP
- **Deployment**: Docker, Docker Compose, Nginx

## Common Development Commands

### Environment Setup
```bash
# Start development dependencies (TimescaleDB + Redis)
docker-compose -f docker-compose.dev.yml up -d

# Install SQLx CLI for database migrations
cargo install sqlx-cli

# Run database migrations
sqlx migrate run

# Create a new migration
sqlx migrate add <migration_name>
```

### Building & Running
```bash
# Development mode
cargo run

# Watch mode with auto-reload (install if needed)
cargo install cargo-watch
cargo watch -x run

# Production build
cargo build --release

# Run tests
cargo test

# Run specific test
cargo test <test_name>

# Run with specific log level
RUST_LOG=debug cargo run
```

### Database Operations
```bash
# Run pending migrations
sqlx migrate run

# Revert last migration
sqlx migrate revert

# Check database status
sqlx migrate info

# Create database if not exists
createdb -U postgres zinnia_dev

# Connect to database
psql -U postgres -d zinnia_dev
```

### Code Quality
```bash
# Check for issues without fixing
cargo clippy -- -D warnings

# Fix clippy issues
cargo clippy --fix -- -D warnings

# Format code
cargo fmt

# Check formatting
cargo fmt -- --check

# Security audit
cargo audit

# Update dependencies
cargo update
```

## Project Structure

### Source Organization
```
src/
├── main.rs                 # Application entry point
├── lib.rs                  # Library exports
├── config/                 # Configuration management
├── db/                     # Database connections (Postgres, Redis)
├── errors/                 # Error types and handling
├── handlers/               # HTTP request handlers
├── middleware/             # Actix middleware (auth, logging, etc.)
├── models/                 # Data structures and database models
├── repositories/           # Data access layer
├── routes/                 # Route configuration
├── security/               # Authentication, encryption, JWT
├── services/               # Business logic layer
├── utils/                  # Utility functions
└── websocket/              # WebSocket handlers
```

### Key Modules
- **Authentication**: `src/security/` - JWT management, API Key handling
- **Database**: `src/db/` - Postgres and Redis connection pools
- **Services**: `src/services/` - Business logic (Auth, Battery, Device, Alert, Notification)
- **Repositories**: `src/repositories/` - Data persistence layer
- **Middleware**: `src/middleware/` - Auth, logging, security headers

## Architecture Patterns

### Layered Architecture
1. **Handlers**: HTTP request/response handling
2. **Services**: Business logic orchestration
3. **Repositories**: Data persistence abstraction
4. **Models**: Data structures and validation

### Authentication Flow
- **Users**: Email/username + password → JWT (access + refresh tokens)
- **Devices**: API Key → JWT exchange (recommended) or direct API Key usage
- **Token Management**: Redis-based blacklist for revocation

### Data Flow
```
Request → Middleware (Auth, Logging) → Handler → Service → Repository → Database
                                                      ↓
                                                Cache (Redis)
```

## Key Workflows

### Device Registration & Data Reporting
1. User creates device via POST `/api/v1/devices`
2. System returns device ID and API Key (store securely!)
3. Device uses API Key to exchange for JWT or reports directly
4. Device reports battery data via POST `/api/v1/battery/report`
5. System evaluates alert rules and triggers notifications

### Alert Management
1. User creates alert rules via POST `/api/v1/alerts/rules`
2. Device reports battery data
3. System evaluates rules against device thresholds
4. Alert events are created and notifications sent
5. Users can acknowledge/resolve alerts

### WebSocket Communication
1. Client connects to `ws(s)://host/ws`
2. Authentication required within 30 seconds
3. Devices can report battery data in real-time
4. Users can subscribe to device data pushes
5. Server pushes battery updates and alerts

## Configuration

### Environment Variables
Required variables (see `.env.example`):
- `DATABASE_URL`: PostgreSQL/TimescaleDB connection
- `REDIS_URL`: Redis connection
- `JWT_SECRET`: JWT signing secret
- `ENCRYPTION_KEY`: Data encryption key
- `SERVER_ADDR`: Bind address (e.g., `0.0.0.0:8080`)

Optional for production:
- `VAPID_PRIVATE_KEY`: Web Push VAPID private key
- `VAPID_PUBLIC_KEY`: Web Push VAPID public key
- `SMTP_*`: Email configuration
- `RECAPTCHA_SECRET`: reCAPTCHA verification

### Configuration Files
- `config/settings.toml`: Application settings
- `config/ssl/`: SSL certificates for HTTPS
- `nginx/`: Nginx configuration

## Testing

### Unit Tests
```bash
# Run unit tests
cargo test --test unit

# Run specific unit test
cargo test <test_function_name>
```

### Integration Tests
```bash
# Run integration tests
cargo test --test integration

# Run with testcontainers (requires Docker)
cargo test --features testcontainers
```

### Test Structure
- **Unit tests**: `tests/unit/` - Individual function testing
- **Integration tests**: `tests/integration/` - API endpoint testing
- **Test helpers**: `tests/helpers/` - Shared test utilities
- **Mocks**: `tests/mocks/` - Mock implementations

## Deployment

### Production Deployment
```bash
# Using deployment script (recommended)
chmod +x scripts/deploy.sh
./scripts/deploy.sh

# Manual deployment
./scripts/preflight-check.sh
./scripts/enable-https.sh
./scripts/generate-vapid-keys.sh
docker-compose -f docker-compose.prod.yml up -d
```

### Docker Commands
```bash
# Build image
docker build -t zinnia:latest .

# Run development stack
docker-compose -f docker-compose.dev.yml up -d

# Run production stack
docker-compose -f docker-compose.prod.yml up -d

# View logs
docker-compose logs -f

# Restart service
docker-compose restart zinnia
```

### SSL/HTTPS
```bash
# Generate certificates with Let's Encrypt
./scripts/enable-https.sh

# Renew certificates
./scripts/renew-ssl.sh
```

## Common Tasks

### Adding a New API Endpoint
1. Define request/response types in `src/models/`
2. Create handler in `src/handlers/`
3. Add business logic to service in `src/services/`
4. Register route in `src/routes/`
5. Add tests in `tests/integration/`

### Database Migrations
1. Create new migration file in `migrations/`
2. Write up.sql and down.sql
3. Run `sqlx migrate run`
4. Test rollback with `sqlx migrate revert`

### Adding Middleware
1. Implement middleware in `src/middleware/`
2. Add to Actix app in `src/main.rs`
3. Configure ordering and scope

### Web Push Setup
1. Generate VAPID keys: `./scripts/generate-vapid-keys.sh`
2. Configure public/private keys in environment
3. Initialize WebPushService in `src/main.rs`
4. Test notification delivery

## Debugging

### Common Issues
- **Database connection failures**: Check TimescaleDB is running and accessible
- **Redis connection failures**: Verify Redis is running with correct password
- **JWT validation errors**: Ensure `JWT_SECRET` is set and consistent
- **Migration failures**: Check SQL syntax and TimescaleDB extensions

### Debug Tools
```bash
# Check database connectivity
sqlx db ping

# View running queries
psql -c "SELECT * FROM pg_stat_activity;"

# Monitor Redis
redis-cli monitor

# Check logs with specific filter
RUST_LOG=zinnia=debug,actix_web=info cargo run
```

### Performance Profiling
```bash
# Run with perf enabled
perf record -g -F 997 cargo run --release

# Generate flamegraph
cargo install flamegraph
cargo flamegraph
```

## Security Considerations

### API Key Management
- API Keys are hashed with Argon2id before storage
- Prefix indexing for efficient lookup
- Support for IP whitelisting and rate limiting
- Automatic tracking of last usage

### JWT Security
- Short-lived access tokens (15 minutes)
- Longer-lived refresh tokens (7 days)
- Redis-based blacklist for revocation
- HMAC-SHA256 signing

### Data Protection
- AES-256-GCM encryption for sensitive data
- Password hashing with Argon2id
- Secure random number generation with Ring
- Rate limiting on all endpoints

## Monitoring & Observability

### Health Checks
- `/health` - Basic health status
- `/health/detailed` - Component status and latency
- `/health/ready` - Kubernetes readiness probe
- `/health/live` - Kubernetes liveness probe

### Metrics
- Request logging with tracing
- Database query performance
- Redis cache hit rates
- JWT validation metrics

### Logging
- Structured JSON logging
- Environment-based log levels
- Request ID tracking
- Error context propagation

## API Reference

### Authentication Endpoints
- `POST /api/v1/auth/exchange` - API Key → JWT exchange
- `POST /api/v1/auth/refresh` - Refresh JWT token
- `POST /api/v1/auth/revoke` - Revoke token

### Device Endpoints
- `POST /api/v1/devices` - Register device
- `GET /api/v1/devices` - List devices
- `PUT /api/v1/devices/{id}/config` - Update device config

### Battery Data Endpoints
- `POST /api/v1/battery/report` - Report battery level
- `POST /api/v1/battery/batch-report` - Batch report
- `GET /api/v1/battery/latest/{device_id}` - Latest reading
- `GET /api/v1/battery/history/{device_id}` - Historical data

### Alert Endpoints
- `POST /api/v1/alerts/rules` - Create alert rule
- `GET /api/v1/alerts/events` - List alert events
- `POST /api/v1/alerts/events/{id}/acknowledge` - Acknowledge alert

## Additional Resources

### Documentation
- `README.md` - Quick start and overview
- `docs/API_REFERENCE.md` - Complete API documentation
- `docs/ARCHITECTURE.md` - System architecture details
- `docs/PRODUCTION_DEPLOYMENT.md` - Production deployment guide
- `docs/TOKEN_GUIDE.md` - Authentication token guide
- `docs/WEB_PUSH_IMPLEMENTATION_SUMMARY.md` - Web Push setup

### Scripts
- `scripts/deploy.sh` - Production deployment
- `scripts/manage.sh` - Service management
- `scripts/preflight-check.sh` - Pre-deployment checks
- `scripts/security-check.sh` - Security validation
- `scripts/generate-vapid-keys.sh` - VAPID key generation

### Configuration
- `.env.example` - Environment variable template
- `.env.production.example` - Production configuration
- `docker-compose.dev.yml` - Development services
- `docker-compose.prod.yml` - Production services
- `nginx/` - Nginx configuration files