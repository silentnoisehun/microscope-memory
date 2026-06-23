#!/bin/bash
# =============================================================================
# Pipeline Proxy — Kezelő script
# Használat: bash pipeline.sh [start|stop|restart|logs|status|keys|models]
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
COMPOSE_FILE="$PROJECT_DIR/references/docker-compose.yml"
CONFIG_FILE="$PROJECT_DIR/references/litellm_config.yaml"
ENV_FILE="$PROJECT_DIR/references/.env"
COMPOSE="docker compose"  # modern; fallback: docker-compose

# Színek
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

# =============================================================================
# Helper függvények
# =============================================================================
info()  { echo -e "${GREEN}[INFO]${NC}  $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }
title() { echo -e "\n${BLUE}══════════════════════════════════════════════════${NC}"; echo -e "${BLUE}  $1${NC}"; echo -e "${BLUE}══════════════════════════════════════════════════${NC}\n"; }

check_env() {
    if [ ! -f "$ENV_FILE" ]; then
        warn ".env fájl nem található: $ENV_FILE"
        warn "Másold a .env.example-öt és töltsd ki a kulcsokat!"
        return 1
    fi
    source "$ENV_FILE" 2>/dev/null || true
    
    # Kötelező kulcsok ellenőrzése
    local missing=0
    for var in LITELLM_MASTER_KEY DATABASE_URL; do
        if [ -z "${!var:-}" ]; then
            error "Hiányzik: $var"
            missing=1
        fi
    done
    
    if [ "$missing" = 1 ]; then
        error "Hiányzó környezeti változók! Töltsd ki a .env fájlt."
        return 1
    fi
    return 0
}

# =============================================================================
# Parancsok
# =============================================================================
cmd_start() {
    title "Pipeline Proxy indítása"
    
    check_env || true
    
    if docker ps --format '{{.Names}}' 2>/dev/null | grep -q "^litellm-pipeline$"; then
        warn "Proxy már fut!"
        $COMPOSE -f "$COMPOSE_FILE" ps
        return 0
    fi
    
    info "LiteLLM konténer indítása..."
    cd "$PROJECT_DIR/references"
    $COMPOSE -f "$COMPOSE_FILE" --env-file "$ENV_FILE" up -d
    
    info "Várakozás a proxy elindulására..."
    for i in $(seq 1 15); do
        if curl -sf http://localhost:4000/health > /dev/null 2>&1; then
            info "✅ Proxy elindult: http://localhost:4000"
            info "   Health:  http://localhost:4000/health"
            info "   Models:  http://localhost:4000/v1/models"
            info "   Admin:   http://localhost:4000 (master key)"
            return 0
        fi
        sleep 2
    done
    
    error "Proxy nem indult el időben! Nézd a naplókat: bash pipeline.sh logs"
    return 1
}

cmd_stop() {
    title "Pipeline Proxy leállítása"
    
    info "LiteLLM konténer leállítása..."
    cd "$PROJECT_DIR/references"
    $COMPOSE -f "$COMPOSE_FILE" down
    
    info "✅ Proxy leállítva"
}

cmd_restart() {
    title "Pipeline Proxy újraindítása"
    cmd_stop
    sleep 2
    cmd_start
}

cmd_logs() {
    cd "$PROJECT_DIR/references"
    $COMPOSE -f "$COMPOSE_FILE" logs --tail=50 -f litellm
}

cmd_status() {
    title "Pipeline Proxy státusz"
    
    cd "$PROJECT_DIR/references"
    
    # Konténer státusz
    if docker ps --format '{{.Names}} {{.Status}}' 2>/dev/null | grep -q "^litellm-pipeline"; then
        local status
        status=$(docker ps --format '{{.Names}} {{.Status}}' | grep "^litellm-pipeline")
        info "✅ Proxy fut: $status"
        
        # Health check
        if curl -sf http://localhost:4000/health > /dev/null 2>&1; then
            info "✅ Health check: OK"
        else
            error "❌ Health check: FAILED"
        fi
        
        # Uptime
        local uptime
        uptime=$(docker inspect --format='{{.State.StartedAt}}' litellm-pipeline 2>/dev/null || echo "ismeretlen")
        info "Indítás: $uptime"
    else
        warn "❌ Proxy nem fut"
        $COMPOSE -f "$COMPOSE_FILE" ps 2>/dev/null || true
    fi
    
    echo ""
    
    # Provider kulcsok ellenőrzése
    source "$ENV_FILE" 2>/dev/null || true
    info "Provider kulcsok:"
    for var in NVIDIA_API_KEY OPENROUTER_API_KEY GEMINI_API_KEY SAMBANOVA_API_KEY; do
        local val="${!var:-}"
        if [ -n "$val" ]; then
            local masked="${val:0:8}...${val: -4}"
            info "  ✅ $var: $masked"
        else
            warn "  ❌ $var: nincs beállítva"
        fi
    done
}

cmd_keys() {
    title "Virtuális API kulcsok"
    
    local master_key="${LITELLM_MASTER_KEY:-}"
    if [ -z "$master_key" ]; then
        source "$ENV_FILE" 2>/dev/null || true
        master_key="${LITELLM_MASTER_KEY:-}"
    fi
    
    if [ -z "$master_key" ]; then
        error "LITELLM_MASTER_KEY nincs beállítva!"
        exit 1
    fi
    
    echo "1) Listázás"
    echo "2) Új kulcs generálása"
    echo "3) Kulcs törlése"
    echo "4) Vissza"
    echo ""
    read -rp "Válassz (1-4): " choice
    
    case $choice in
        1)
            curl -s http://localhost:4000/key/list \
                -H "Authorization: Bearer $master_key" | jq .
            ;;
        2)
            read -rp "Kliens neve (cline/kilo/opencode/openclaw): " user
            read -rp "Model csoport (gyors/eros/ultra/mind): " models
            local model_arg
            if [ "$models" = "mind" ]; then
                model_arg='["gyors", "eros", "ultra"]'
            else
                model_arg="[\"$models\"]"
            fi
            
            local result
            result=$(curl -s -X POST http://localhost:4000/key/generate \
                -H "Authorization: Bearer $master_key" \
                -H "Content-Type: application/json" \
                -d "{
                    \"models\": $model_arg,
                    \"metadata\": {\"user\": \"$user\"},
                    \"max_budget\": 10.0,
                    \"budget_duration\": \"30d\"
                }")
            
            local key
            key=$(echo "$result" | jq -r '.key // "hiba"')
            info "Kulcs generálva: $key"
            info "Másold ki! (nem lesz újra elérhető)"
            ;;
        3)
            read -rp "Törlendő kulcs: " del_key
            curl -s -X POST http://localhost:4000/key/delete \
                -H "Authorization: Bearer $master_key" \
                -H "Content-Type: application/json" \
                -d "{\"key\": \"$del_key\"}" | jq .
            ;;
        4)
            return 0
            ;;
    esac
}

cmd_models() {
    title "Elérhető modellek"
    
    local master_key="${LITELLM_MASTER_KEY:-}"
    if [ -z "$master_key" ]; then
        source "$ENV_FILE" 2>/dev/null || true
        master_key="${LITELLM_MASTER_KEY:-}"
    fi
    
    if [ -z "$master_key" ]; then
        error "LITELLM_MASTER_KEY nincs beállítva!"
        exit 1
    fi
    
    curl -s http://localhost:4000/v1/models \
        -H "Authorization: Bearer $master_key" | jq '.data[] | {id, name}'
}

cmd_spend() {
    title "Költség kimutatás"
    
    local master_key="${LITELLM_MASTER_KEY:-}"
    if [ -z "$master_key" ]; then
        source "$ENV_FILE" 2>/dev/null || true
        master_key="${LITELLM_MASTER_KEY:-}"
    fi
    
    if [ -z "$master_key" ]; then
        error "LITELLM_MASTER_KEY nincs beállítva!"
        exit 1
    fi
    
    curl -s http://localhost:4000/spend/keys \
        -H "Authorization: Bearer $master_key" | jq '.'
}

# =============================================================================
# Main
# =============================================================================
case "${1:-help}" in
    start)
        cmd_start
        ;;
    stop)
        cmd_stop
        ;;
    restart)
        cmd_restart
        ;;
    logs)
        cmd_logs
        ;;
    status)
        cmd_status
        ;;
    keys)
        cmd_keys
        ;;
    models)
        cmd_models
        ;;
    spend)
        cmd_spend
        ;;
    help|*)
        title "Pipeline Proxy — Használat"
        echo "  start     — Proxy indítása"
        echo "  stop      — Proxy leállítása"
        echo "  restart   — Proxy újraindítása"
        echo "  logs      — Naplók követése"
        echo "  status    — Státusz ellenőrzés"
        echo "  keys      — Virtuális API kulcsok kezelése"
        echo "  models    — Elérhető modellek listázása"
        echo "  spend     — Költség kimutatás"
        echo ""
        echo "Példák:"
        echo "  bash pipeline.sh start"
        echo "  bash pipeline.sh keys"
        echo "  bash pipeline.sh status"
        echo ""
        exit 0
        ;;
esac
