#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

if [[ ! -f .env ]]; then
  cp .env.example .env
  echo "Created .env from .env.example. Edit it before first production run."
fi

mkdir -p ./logs/caddy ./data/layers ./data/output ./backups
chmod +x ./one-click-cloud.sh ./easy-start.sh ./bootstrap-ubuntu.sh ./backup.sh ./restore.sh ./install_fail2ban.sh

docker compose up -d --build

echo "Deployment started. Check status with: docker compose ps"
