#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
  echo "Run as root: sudo bash install_fail2ban.sh"
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_FILTER="/etc/fail2ban/filter.d/caddy-auth.conf"
TARGET_JAIL="/etc/fail2ban/jail.d/caddy-auth.local"

apt-get update
apt-get install -y fail2ban jq

install -m 0644 "${SCRIPT_DIR}/fail2ban/filter.d/caddy-auth.conf" "${TARGET_FILTER}"
install -m 0644 "${SCRIPT_DIR}/fail2ban/jail.d/caddy-auth.local" "${TARGET_JAIL}"

systemctl enable fail2ban
systemctl restart fail2ban
fail2ban-client status caddy-auth

echo "fail2ban installed and caddy-auth jail enabled"