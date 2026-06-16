#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

if ! command -v docker >/dev/null 2>&1; then
  echo "Docker nincs telepitve. Telepitsd, majd futtasd ujra."
  exit 1
fi

if [[ ! -f .env ]]; then
  cp .env.example .env
fi

read -rp "Domain (pl. memory.example.com): " DOMAIN
read -rp "Email (Let's Encrypt): " ACME_EMAIL
read -rp "Felhasznalonev (basic auth): " BASIC_AUTH_USER
read -rsp "Jelszo (basic auth): " BASIC_AUTH_PASSWORD
echo
read -rp "Backup retention napok [14]: " BACKUP_RETENTION_DAYS
BACKUP_RETENTION_DAYS="${BACKUP_RETENTION_DAYS:-14}"

if [[ -z "${DOMAIN}" || -z "${ACME_EMAIL}" || -z "${BASIC_AUTH_USER}" || -z "${BASIC_AUTH_PASSWORD}" ]]; then
  echo "Minden mezo kotelezo."
  exit 1
fi

echo "Jelszo hash keszitese..."
BASIC_AUTH_HASH="$(docker run --rm caddy:2.8 caddy hash-password --plaintext "${BASIC_AUTH_PASSWORD}")"

cat > .env <<EOF
DOMAIN=${DOMAIN}
ACME_EMAIL=${ACME_EMAIL}
BASIC_AUTH_USER=${BASIC_AUTH_USER}
BASIC_AUTH_HASH=${BASIC_AUTH_HASH}
BACKUP_RETENTION_DAYS=${BACKUP_RETENTION_DAYS}
EOF

echo "Deploy indul..."
bash ./deploy.sh

echo
echo "Kesz. Ellenorzes:" 
echo "  docker compose ps"
echo "  curl -u \"${BASIC_AUTH_USER}:<jelszo>\" https://${DOMAIN}/v1/status"
echo
echo "Opcionis hardening:" 
echo "  sudo bash ./install_fail2ban.sh"
echo "  sudo cp systemd/microscope-memory-backup.service /etc/systemd/system/"
echo "  sudo cp systemd/microscope-memory-backup.timer /etc/systemd/system/"
echo "  sudo systemctl daemon-reload && sudo systemctl enable --now microscope-memory-backup.timer"