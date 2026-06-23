#!/usr/bin/env bash
# ════════════════════════════════════════════════════════════════════════════
# install.sh — Install Microscope Memory shorthand + optional PATH entry
# ════════════════════════════════════════════════════════════════════════════
#
# Usage:
#   ./scripts/install.sh                # install 'mm' to ~/.local/bin (or /usr/local/bin if writable)
#   ./scripts/install.sh --uninstall    # remove the symlinks we created
#   ./scripts/install.sh --bin-link     # also create a 'microscope-mem' symlink
#   ./scripts/install.sh --prefix DIR   # install to DIR instead of ~/.local/bin
#   ./scripts/install.sh --no-env       # skip writing env file to shell rc
#
# What it does:
#   1. Symlinks 'mm' from this repo's scripts/ into the chosen bin dir
#   2. Optionally symlinks 'microscope-mem' to the local target/release binary
#   3. Writes ~/.config/microscope/env.sh with documented env vars
#   4. Adds a 'source' line to ~/.bashrc (and ~/.zshrc if present) unless --no-env
#
# ════════════════════════════════════════════════════════════════════════════

set -e

# ─── Defaults ───────────────────────────────────────────────────────────────
PREFIX="${HOME}/.local/bin"
WRITE_ENV=1
CREATE_BIN_LINK=0
UNINSTALL=0

# ─── Parse args ─────────────────────────────────────────────────────────────
while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)    PREFIX="$2"; shift 2 ;;
        --no-env)    WRITE_ENV=0; shift ;;
        --bin-link)  CREATE_BIN_LINK=1; shift ;;
        --uninstall) UNINSTALL=1; shift ;;
        -h|--help)
            sed -n '2,20p' "$0"
            exit 0
            ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

# ─── Resolve script dir ─────────────────────────────────────────────────────
SOURCE="${BASH_SOURCE[0]}"
while [ -h "$SOURCE" ]; do
    DIR=$(cd -P "$(dirname "$SOURCE")" && pwd)
    SOURCE=$(readlink "$SOURCE")
    [[ "$SOURCE" != /* ]] && SOURCE="$DIR/$SOURCE"
done
SCRIPT_DIR=$(cd -P "$(dirname "$SOURCE")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

# ─── Uninstall mode ─────────────────────────────────────────────────────────
if [ "$UNINSTALL" = "1" ]; then
    echo "Uninstalling Microscope Memory shorthand..."
    [ -L "$PREFIX/mm" ] && rm -v "$PREFIX/mm" && echo "  removed $PREFIX/mm"
    [ -L "$PREFIX/microscope-mem" ] && rm -v "$PREFIX/microscope-mem" && echo "  removed $PREFIX/microscope-mem"
    ENV_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/microscope/env.sh"
    [ -f "$ENV_FILE" ] && rm -v "$ENV_FILE" && echo "  removed $ENV_FILE"
    # Remove source lines from shell rc (best-effort)
    for rc in "$HOME/.bashrc" "$HOME/.zshrc"; do
        if [ -f "$rc" ] && grep -q "microscope/env.sh" "$rc" 2>/dev/null; then
            # Remove lines that source the env file
            tmp=$(mktemp)
            grep -v "microscope/env.sh" "$rc" > "$tmp" || true
            mv "$tmp" "$rc"
            echo "  cleaned $rc"
        fi
    done
    echo "Done."
    exit 0
fi

# ─── Ensure prefix exists ───────────────────────────────────────────────────
if ! mkdir -p "$PREFIX" 2>/dev/null; then
    echo "Error: cannot create $PREFIX (try --prefix DIR or run with permissions)" >&2
    exit 1
fi

# ─── Symlink 'mm' ───────────────────────────────────────────────────────────
if [ -e "$PREFIX/mm" ] && [ ! -L "$PREFIX/mm" ]; then
    echo "Warning: $PREFIX/mm exists and is not a symlink. Skipping." >&2
else
    ln -sf "$SCRIPT_DIR/mm" "$PREFIX/mm"
    echo "  linked $PREFIX/mm → $SCRIPT_DIR/mm"
fi

# ─── Optional: symlink 'microscope-mem' to the local binary ─────────────────
if [ "$CREATE_BIN_LINK" = "1" ]; then
    exe="microscope-mem"
    case "$(uname -s 2>/dev/null || echo Windows)" in
        MINGW*|CYGWIN*|MSYS*|Windows*) exe="microscope-mem.exe" ;;
    esac
    BIN_SRC="$REPO_ROOT/target/release/$exe"
    if [ ! -x "$BIN_SRC" ]; then
        echo "Warning: $BIN_SRC not found. Build first with: cargo build --release" >&2
    else
        if [ -e "$PREFIX/microscope-mem" ] && [ ! -L "$PREFIX/microscope-mem" ]; then
            echo "Warning: $PREFIX/microscope-mem exists and is not a symlink. Skipping." >&2
        else
            ln -sf "$BIN_SRC" "$PREFIX/microscope-mem"
            echo "  linked $PREFIX/microscope-mem → $BIN_SRC"
        fi
    fi
fi

# ─── Write env file ─────────────────────────────────────────────────────────
if [ "$WRITE_ENV" = "1" ]; then
    ENV_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/microscope"
    ENV_FILE="$ENV_DIR/env.sh"
    mkdir -p "$ENV_DIR"

    # Don't clobber user customizations. If the file already has any of our
    # env vars, leave it alone and just print a hint.
    SHOULD_WRITE=1
    if [ -f "$ENV_FILE" ] && grep -qE '^export MICROSCOPE_' "$ENV_FILE" 2>/dev/null; then
        SHOULD_WRITE=0
    fi

    if [ "$SHOULD_WRITE" = "1" ]; then
        cat > "$ENV_FILE" <<EOF
# ============================================================================
# Microscope Memory - environment configuration
# Sourced automatically by your shell rc (added by install.sh).
# ============================================================================

# Required: full path to the microscope-mem binary. The mm script will also
# auto-discover via \$MICROSCOPE_HOME/target/release or your \$PATH, so this
# is only needed if you keep the binary outside the repo.
# export MICROSCOPE_BIN="$REPO_ROOT/target/release/microscope-mem"

# Optional: project root (binary lives in \$MICROSCOPE_HOME/target/release)
# export MICROSCOPE_HOME="$REPO_ROOT"

# Optional: defaults for the mm shorthand
export MICROSCOPE_DEFAULT_LAYER="\${MICROSCOPE_DEFAULT_LAYER:-long_term}"
export MICROSCOPE_DEFAULT_K="\${MICROSCOPE_DEFAULT_K:-3}"
export MICROSCOPE_DEFAULT_IMPORT="\${MICROSCOPE_DEFAULT_IMPORT:-5}"

# Optional: hook behaviour
# export MICROSCOPE_STOP_MODE="transcript"   # transcript | session_file | none
# export MICROSCOPE_RECALL_K="5"
# export MICROSCOPE_QUIET="0"
EOF
        echo "  wrote $ENV_FILE"
    else
        echo "  kept existing $ENV_FILE (already has MICROSCOPE_* exports)"
    fi

    # Add to shell rc (idempotent)
    for rc in "$HOME/.bashrc" "$HOME/.zshrc"; do
        if [ -f "$rc" ] && ! grep -q "microscope/env.sh" "$rc" 2>/dev/null; then
            printf '\n# Microscope Memory shorthand\n[ -f "%s" ] && source "%s"\n' "$ENV_FILE" "$ENV_FILE" >> "$rc"
            echo "  appended source line to $rc"
        fi
    done
fi

# ─── PATH check & auto-add ──────────────────────────────────────────────────
case ":$PATH:" in
    *":$PREFIX:"*) ;;
    *)
        # Try to append the export to the user's bashrc so 'mm' works next session.
        ADDED=0
        if [ -w "$HOME/.bashrc" ] 2>/dev/null && ! grep -q "$PREFIX" "$HOME/.bashrc" 2>/dev/null; then
            printf '\n# Microscope Memory shorthand - add bin to PATH\nexport PATH="%s:$PATH"\n' "$PREFIX" >> "$HOME/.bashrc"
            echo "  appended PATH export to ~/.bashrc"
            ADDED=1
        fi
        if [ "$ADDED" = "0" ]; then
            echo ""
            echo "Note: $PREFIX is not in your PATH. Add this to your shell:"
            echo "  export PATH=\"$PREFIX:\$PATH\""
        fi
        ;;
esac

echo ""
echo "Done. Test with:  mm h"
echo "Uninstall with:  $0 --uninstall"
echo ""
echo "Optional: generate .claude/settings.json with the correct hook path:"
echo "  $SCRIPT_DIR/install-claude-hooks.sh"
