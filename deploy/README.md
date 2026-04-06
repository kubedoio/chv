# CHV Deployment Guide

This directory contains Dockerfiles and deployment configurations for CHV services.

## Quick Start with Docker Compose

1. **Start the core services:**
   ```bash
   docker compose up -d
   ```

2. **Start with WebUI:**
   ```bash
   docker compose --profile with-ui up -d
   ```

3. **Start with Agent (for testing):**
   ```bash
   docker compose --profile with-agent up -d
   ```

## Configuration

### Using Config Files

The services support YAML configuration files:

1. **Controller config:** `configs/controller.yaml`
   ```yaml
   http_addr: ":8080"
   grpc_addr: ":9090"
   database_url: "postgres://chv:chv@localhost:5432/chv?sslmode=disable"
   log_level: "info"
   
   cors:
     allowed_origins:
       - "http://10.5.199.83:3000"
       - "http://localhost:3000"
   ```

2. **Agent config:** `configs/agent.yaml`
   ```yaml
   node_id: "agent-01"
   controller_addr: "localhost:9090"
   data_dir: "/var/lib/chv-agent"
   ```

### Using Environment Variables

All settings can be configured via environment variables:

```bash
# Controller
export CHV_HTTP_ADDR=:8080
export CHV_DATABASE_URL=postgres://chv:chv@localhost:5432/chv
export CHV_CORS_ORIGINS=http://10.5.199.83:3000,http://localhost:3000

# Agent
export CHV_NODE_ID=agent-01
export CHV_CONTROLLER_ADDR=localhost:9090
export CHV_DATA_DIR=/var/lib/chv-agent
```

### Using .env File

Create a `.env` file in the project root:

```bash
cp .env.example .env
# Edit .env with your settings
```

## Docker Images

### Building Images

```bash
# Controller
docker build -f deploy/controller.Dockerfile -t chv-controller:latest .

# Agent
docker build -f deploy/agent.Dockerfile -t chv-agent:latest .

# Bootstrap
docker build -f deploy/bootstrap.Dockerfile -t chv-bootstrap:latest .
```

### Running Containers

**Controller:**
```bash
docker run -d \
  -p 8080:8080 \
  -p 9090:9090 \
  -e CHV_DATABASE_URL=postgres://... \
  -v /etc/chv/controller.yaml:/etc/chv/controller.yaml:ro \
  chv-controller:latest \
  -config /etc/chv/controller.yaml
```

**Agent:**
```bash
docker run -d \
  --privileged \
  -v /dev/kvm:/dev/kvm \
  -v /var/lib/chv-agent:/var/lib/chv-agent \
  -e CHV_CONTROLLER_ADDR=controller:9090 \
  chv-agent:latest
```

## Production Deployment

### 1. Database Setup

```bash
# Run database migrations
docker run --rm \
  -e DATABASE_URL=postgres://... \
  chv-controller:latest \
  migrate up
```

### 2. Generate Secrets

```bash
# Generate JWT secret
export CHV_JWT_SECRET=$(openssl rand -base64 32)
```

### 3. Configure CORS

For production, set the exact WebUI origin:

```yaml
# controller.yaml
cors:
  allowed_origins:
    - "https://your-domain.com"
    - "https://admin.your-domain.com"
  allow_credentials: true
```

### 4. SSL/TLS

Use a reverse proxy (nginx, traefik) for SSL termination:

```nginx
server {
    listen 443 ssl;
    server_name api.your-domain.com;
    
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    
    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

## Troubleshooting

### CORS Errors

If you see "Cross-Origin Request Blocked":

1. Check the WebUI origin is in `cors.allowed_origins`
2. Verify the controller is using the correct config file
3. Check browser console for exact origin being used

### Database Connection Issues

```bash
# Test database connection
docker compose exec controller pg_isready -h postgres -U chv
```

### Agent Not Connecting

1. Verify agent can reach controller gRPC port
2. Check firewall rules for port 9090
3. Review agent logs: `docker compose logs agent`
