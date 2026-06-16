#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
  echo "Futtasd rootkent: sudo bash bootstrap-ubuntu.sh"
  exit 1
fi

REPO_DIR="/opt/microscope-memory"
REPO_URL="https://github.com/silentnoisehun/microscope-memory.git"

apt-get update
apt-get install -y ca-certificates curl gnupg lsb-release git

if ! command -v docker >/dev/null 2>&1; then
  install -m 0755 -d /etc/apt/keyrings
  curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
  chmod a+r /etc/apt/keyrings/docker.gpg
  echo \
    "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu \
    $(. /etc/os-release && echo \"$VERSION_CODENAME\") stable" | \
    tee /etc/apt/sources.list.d/docker.list >/dev/null
  apt-get update
  apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
fi

if [[ ! -d "${REPO_DIR}" ]]; then
  git clone "${REPO_URL}" "${REPO_DIR}"
else
  git -C "${REPO_DIR}" pull --ff-only
fi

cd "${REPO_DIR}/.github/cloud-deploy"
chmod +x ./one-click-cloud.sh ./easy-start.sh ./deploy.sh ./backup.sh ./restore.sh ./install_fail2ban.sh

echo "Bootstrap kesz. Inditsd ezt:"
echo "  cd ${REPO_DIR}/.github/cloud-deploy"
echo "  bash one-click-cloud.sh"
