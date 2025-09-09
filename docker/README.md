# Docker Deployment Guide

This directory contains Docker configuration files for deploying Nexus in containerized environments.

## Quick Start

1. **Copy environment template:**
   ```bash
   cp .env.example .env
   ```

2. **Configure environment variables in `.env`:**
   - Add your LLM provider API keys (OpenAI, Anthropic, Google, etc.)
   - Set up MCP server tokens (GitHub, etc.)
   - Configure optional services (OAuth2, Redis, telemetry)

3. **Start basic services:**
   ```bash
   docker-compose up -d
   ```

4. **Check health:**
   ```bash
   curl http://localhost:8000/health
   ```

## Service Profiles

### Basic Setup (Default)
```bash
docker-compose up
```
- Nexus application
- Redis for distributed rate limiting

### With OAuth2 Testing
```bash
docker-compose --profile oauth up
```
- Adds Hydra OAuth2 server for development/testing

### With Observability
```bash
docker-compose --profile observability up
```
- Adds OpenTelemetry collector for metrics

### Full Setup
```bash
docker-compose --profile oauth --profile observability up
```

## Configuration

### Environment Variables

Key variables you'll need to set in `.env`:

- **LLM Providers**: `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GOOGLE_API_KEY`
- **AWS Bedrock**: `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`
- **MCP Servers**: `GITHUB_TOKEN`
- **Ports**: `NEXUS_PORT` (default: 8000)

### Nexus Configuration

The `nexus.toml` file configures:
- Server settings and health checks
- LLM provider configurations and models
- MCP server connections
- Rate limiting policies
- Telemetry and observability

### Volume Mounts

- `nexus_data`: Application data persistence
- `redis_data`: Redis data persistence
- `hydra-sqlite`: OAuth2 server data (when using oauth profile)

## Production Deployment

### Security Checklist

- [ ] Use strong, unique API keys
- [ ] Enable OAuth2 authentication (`server.oauth` in `nexus.toml`)
- [ ] Configure TLS/SSL termination (reverse proxy recommended)
- [ ] Set up proper firewall rules
- [ ] Use secrets management (Docker Secrets, Kubernetes Secrets)
- [ ] Enable security scanning for container images

### Scaling Considerations

- **Redis**: Use external Redis cluster for production
- **Load Balancing**: Deploy multiple Nexus instances behind load balancer
- **Monitoring**: Enable telemetry and use external observability stack
- **Backup**: Backup Redis data and configuration files

### Example Production Override

Create `docker-compose.prod.yml`:

```yaml
version: '3.8'
services:
  nexus:
    restart: always
    deploy:
      replicas: 3
      resources:
        limits:
          memory: 1G
        reservations:
          memory: 512M
    environment:
      - TELEMETRY_SERVICE_NAME=nexus-prod
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
```

Deploy with:
```bash
docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d
```

## Troubleshooting

### Common Issues

1. **Port conflicts**: Change `NEXUS_PORT` in `.env`
2. **Missing API keys**: Check `.env` file configuration
3. **Redis connection**: Ensure Redis service is healthy
4. **Health check failures**: Check logs with `docker-compose logs nexus`

### Debug Commands

```bash
# View logs
docker-compose logs -f nexus

# Check service health
docker-compose ps

# Connect to running container
docker-compose exec nexus /bin/bash

# Test Redis connection
docker-compose exec redis redis-cli ping

# Check configuration
docker-compose exec nexus cat /etc/nexus.toml
```

### Performance Monitoring

Monitor these metrics:
- Container resource usage
- Response times via health endpoint
- Redis memory usage
- Rate limit hit rates (via telemetry)

## Development

### Local Development with Docker

```bash
# Build and run for development
docker-compose -f docker-compose.yml up --build

# Run with auto-rebuild
docker-compose up --build nexus
```

### Integration Testing

The integration tests use a separate Docker Compose setup in `crates/integration-tests/`:

```bash
cd crates/integration-tests
docker-compose up -d
cd ../..
cargo nextest run -p integration-tests
```