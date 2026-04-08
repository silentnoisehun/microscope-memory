# Cloud Deploy Option (Microscope-Memory)

This folder gives you a production-ready cloud deployment with HTTPS, auth, hardening, and automated backups.

## Fastest path (recommended)
One-click start:
```bash
cd /opt/microscope-memory/.github/cloud-deploy
bash one-click-cloud.sh
```

This script runs bootstrap + wizard + deploy in one flow.

Windows launcher (from this folder):
```bat
one-click-cloud.bat
```
Note: this requires WSL (`wsl --install`).

If you want to run steps manually (Ubuntu):
```bash
cd /opt/microscope-memory/.github/cloud-deploy
sudo bash bootstrap-ubuntu.sh
bash easy-start.sh
```

## What you get
- `microscope-memory` container running Bridge API (`:6060` internal only)
- `caddy` reverse proxy with automatic TLS (Let's Encrypt)
- HTTP Basic authentication at the edge
- Security headers and narrowed route exposure (`/v1/*`, OpenAPI)
- Persistent storage in `./data/layers` and `./data/output`
- Local backup script with retention
- Optional fail2ban for repeated auth failures
- Optional IP allowlist Caddy profile

## 1) Prepare server
- Linux VM with public IP
- DNS `A` record: domain -> VM IP
- Docker + Docker Compose installed
- Recommended checkout path: `/opt/microscope-memory`

## 2) Configure environment
```bash
cd /opt/microscope-memory/.github/cloud-deploy
cp .env.example .env
```

Generate password hash:
```bash
docker run --rm caddy:2.8 caddy hash-password --plaintext "your-strong-password"
```

Set in `.env`:
- `DOMAIN`
- `ACME_EMAIL`
- `BASIC_AUTH_USER`
- `BASIC_AUTH_HASH`
- `BACKUP_RETENTION_DAYS`

## 3) Start stack
```bash
cd /opt/microscope-memory/.github/cloud-deploy
bash deploy.sh
```

Health checks:
```bash
docker compose ps
curl -u "<user>:<password>" https://<your-domain>/v1/status
```

## 4) Optional IP allowlist
Default `Caddyfile` is public (with auth). To restrict to private ranges or your own CIDRs:
```bash
cd /opt/microscope-memory/.github/cloud-deploy
cp Caddyfile.allowlist Caddyfile
# edit CIDRs in Caddyfile.allowlist section (@not_allowed matcher)
docker compose restart caddy
```

## 5) Optional fail2ban (auth brute-force protection)
```bash
cd /opt/microscope-memory/.github/cloud-deploy
sudo bash install_fail2ban.sh
```

Important: update log path in `fail2ban/jail.d/caddy-auth.local` if your checkout is not `/opt/microscope-memory`.

Quick checks:
```bash
sudo fail2ban-client status
sudo fail2ban-client status caddy-auth
```

## 6) Automated backups
Manual backup:
```bash
bash /opt/microscope-memory/.github/cloud-deploy/backup.sh
```

Restore from archive:
```bash
bash /opt/microscope-memory/.github/cloud-deploy/restore.sh /opt/microscope-memory/.github/cloud-deploy/backups/microscope-memory_YYYYMMDD_HHMMSS.tar.gz
```

Systemd timer setup:
```bash
sudo cp systemd/microscope-memory-backup.service /etc/systemd/system/
sudo cp systemd/microscope-memory-backup.timer /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now microscope-memory-backup.timer
sudo systemctl list-timers | grep microscope-memory-backup
```

Alternative cron setup:
- See `cron.example` and add it with `crontab -e`.

## 7) Client integrations
- ChatGPT Action: use `https://<your-domain>/...` OpenAPI endpoints
- Ollama sidecar: call the same HTTPS endpoint
- Claude integrations can use MCP bridge that proxies HTTPS API
- OpenClaw integration pack:
```bash
cd /opt/microscope-memory/.github/openclaw
bash install-openclaw-integration.sh
```

## Notes
- For private-only access, prefer WireGuard/Tailscale + allowlist profile.
- Basic auth is good baseline, but OAuth/OIDC gateway is better for teams.
