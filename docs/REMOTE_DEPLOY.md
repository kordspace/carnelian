# Remote Deployment Guide

This guide covers deploying Carnelian OS to remote servers for access outside your local machine.

## Prerequisites

- A VPS or cloud server (Linux recommended)
- Docker and Docker Compose installed
- Domain name (optional but recommended)
- TLS certificate (Let's Encrypt recommended)

## Quick Start

### 1. Clone and Configure

```bash
git clone https://github.com/kordspace/carnelian.git
cd carnelian
cp machine.toml.example machine.toml
```

Edit `machine.toml` with your settings:

```toml
machine_profile = "thummim"
database_url = "postgres://carnelian:your_password@localhost:5432/carnelian"
gateway_url = "http://localhost:18790"
http_port = 18789
```

### 2. Get Your API Key

Run locally first to get the API key:

```bash
carnelian status
# Or call the API directly:
curl http://localhost:18789/v1/config/api-key
```

Save this key — you'll need it for all remote requests.

### 3. Deploy with Docker

```bash
# Start PostgreSQL and Carnelian
docker-compose up -d postgres
docker-compose up -d carnelian
```

### 4. Configure Reverse Proxy

See configs below for Nginx or Caddy.

## Deployment Options

### Fly.io (Recommended for Simplicity)

Create `fly.toml`:

```toml
app = "your-carnelian-app"
primary_region = "iad"

[build]
  dockerfile = "Dockerfile"

[env]
  DATABASE_URL = "postgres://..."
  RUST_LOG = "info"

[[services]]
  internal_port = 18789
  protocol = "tcp"

  [[services.ports]]
    handlers = ["http"]
    port = 80

  [[services.ports]]
    handlers = ["tls", "http"]
    port = 443

[mounts]
  source = "carnelian_data"
  destination = "/data"
```

Deploy:

```bash
fly deploy
```

### VPS with Docker Compose

On your server:

```bash
cd /opt/carnelian
docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d
```

### Systemd Service

Create `/etc/systemd/system/carnelian.service`:

```ini
[Unit]
Description=Carnelian OS
After=network.target docker.service
Requires=docker.service

[Service]
Type=simple
WorkingDirectory=/opt/carnelian
ExecStart=/usr/bin/docker-compose up
ExecStop=/usr/bin/docker-compose down
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable:

```bash
systemctl enable carnelian
systemctl start carnelian
```

## Reverse Proxy Configs

### Nginx with SSL

See `docs/deploy/nginx.conf` for a complete example:

```nginx
server {
    listen 443 ssl http2;
    server_name your-domain.com;

    ssl_certificate /etc/letsencrypt/live/your-domain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/your-domain.com/privkey.pem;

    location / {
        proxy_pass http://localhost:18789;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### Caddy (Automatic HTTPS)

See `docs/deploy/Caddyfile`:

```caddy
your-domain.com {
    reverse_proxy localhost:18789
}
```

## Remote Client Configuration

When connecting remotely, include the `X-Carnelian-Key` header:

```bash
curl -H "X-Carnelian-Key: your-api-key" \
  https://your-domain.com/v1/status
```

For the UI, you can:
1. Access via web browser at `https://your-domain.com`
2. Build and serve web UI from the server:

```bash
# Build web UI
cargo install dioxus-cli
dx build --platform web -p carnelian-ui --release

# Then start core (serves /ui automatically)
carnelian start
```

## Security Considerations

- Always use HTTPS in production
- Keep your API key secret
- Use strong PostgreSQL passwords
- Enable firewall rules (allow 443, block 18789 from external)
- Consider VPN or Tailscale for additional security

## Troubleshooting

**Connection refused**: Check firewall rules and binding address

**TLS errors**: Verify certificate paths and renewal

**API key rejected**: Regenerate via `carnelian status` or health endpoint

## Next Steps

- Set up automated backups
- Configure monitoring (Prometheus/Grafana)
- Set up log aggregation
- Enable safe mode for production
