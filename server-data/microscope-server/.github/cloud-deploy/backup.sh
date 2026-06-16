#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKUP_DIR="${ROOT_DIR}/backups"
ENV_FILE="${ROOT_DIR}/.env"
TS="$(date +"%Y%m%d_%H%M%S")"
RETENTION_DAYS=14

if [[ -f "${ENV_FILE}" ]]; then
  # shellcheck disable=SC1090
  source "${ENV_FILE}"
  RETENTION_DAYS="${BACKUP_RETENTION_DAYS:-14}"
fi

mkdir -p "${BACKUP_DIR}"

ARCHIVE="${BACKUP_DIR}/microscope-memory_${TS}.tar.gz"
tar -czf "${ARCHIVE}" -C "${ROOT_DIR}" data config.toml .env

find "${BACKUP_DIR}" -type f -name "microscope-memory_*.tar.gz" -mtime "+${RETENTION_DAYS}" -delete

echo "Backup created: ${ARCHIVE}"
echo "Retention policy: ${RETENTION_DAYS} days"