#!/usr/bin/env bash
set -euo pipefail

# Microscope-Memory API helper for OpenClaw skills.
# Required env vars:
#   MICROSCOPE_BASE_URL (e.g. https://memory.example.com)
#   MICROSCOPE_USER
#   MICROSCOPE_PASS
#
# Usage:
#   microscope-memory-api.sh status
#   microscope-memory-api.sh recall "query text" [k]
#   microscope-memory-api.sh remember "text" [layer] [importance]

require_env() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    echo "Missing env var: ${name}" >&2
    exit 2
  fi
}

json_escape() {
  printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e ':a;N;$!ba;s/\n/\\n/g'
}

api_get() {
  local path="$1"
  shift
  curl -fsS -u "${MICROSCOPE_USER}:${MICROSCOPE_PASS}" "$@" "${MICROSCOPE_BASE_URL}${path}"
}

api_post_json() {
  local path="$1"
  local payload="$2"
  curl -fsS -u "${MICROSCOPE_USER}:${MICROSCOPE_PASS}" \
    -H "Content-Type: application/json" \
    -X POST \
    -d "${payload}" \
    "${MICROSCOPE_BASE_URL}${path}"
}

require_env MICROSCOPE_BASE_URL
require_env MICROSCOPE_USER
require_env MICROSCOPE_PASS

cmd="${1:-}"
if [[ -z "${cmd}" ]]; then
  echo "Usage: $0 {status|recall|remember} ..." >&2
  exit 2
fi

case "${cmd}" in
  status)
    api_get "/v1/status"
    ;;

  recall)
    query="${2:-}"
    k="${3:-10}"
    if [[ -z "${query}" ]]; then
      echo "Usage: $0 recall \"query\" [k]" >&2
      exit 2
    fi

    curl -fsS -u "${MICROSCOPE_USER}:${MICROSCOPE_PASS}" \
      --get \
      --data-urlencode "q=${query}" \
      --data-urlencode "k=${k}" \
      "${MICROSCOPE_BASE_URL}/v1/recall"
    ;;

  remember)
    text="${2:-}"
    layer="${3:-long_term}"
    importance="${4:-7}"
    if [[ -z "${text}" ]]; then
      echo "Usage: $0 remember \"text\" [layer] [importance]" >&2
      exit 2
    fi

    text_escaped="$(json_escape "${text}")"
    layer_escaped="$(json_escape "${layer}")"
    payload="{\"text\":\"${text_escaped}\",\"layer\":\"${layer_escaped}\",\"importance\":${importance}}"
    api_post_json "/v1/remember" "${payload}"
    ;;

  *)
    echo "Unknown command: ${cmd}" >&2
    exit 2
    ;;
esac