#!/bin/sh
# install.sh — bootstrap the `agbot` CLI on a Linux server or a Mac.
#
#   curl -fsSL https://raw.githubusercontent.com/vivasaayi/agbot/main/scripts/install.sh | sh
#
# It clones (or updates) the AGBot repo into $AGBOT_HOME (default ~/.agbot/repo)
# and links the `agbot` command onto your PATH. Afterwards:
#
#   Linux or Mac: agbot up            # start the farmer app with Docker
#                 open http://127.0.0.1:8080/portal
#
# Environment overrides:
#   AGBOT_HOME   install root            (default: ~/.agbot)
#   AGBOT_REPO   git URL to clone        (default: https://github.com/vivasaayi/agbot.git)
#   AGBOT_REF    branch/tag to check out (default: main)
#   BIN_DIR      where to link `agbot`   (default: first writable of /usr/local/bin, ~/.local/bin)
set -eu

AGBOT_HOME=${AGBOT_HOME:-$HOME/.agbot}
AGBOT_REPO=${AGBOT_REPO:-https://github.com/vivasaayi/agbot.git}
AGBOT_REF=${AGBOT_REF:-main}
REPO_DIR="$AGBOT_HOME/repo"

log() { printf '\033[1;36m[install]\033[0m %s\n' "$*" >&2; }
die() { printf '\033[1;31m[install]\033[0m %s\n' "$*" >&2; exit 1; }

command -v git >/dev/null 2>&1 || die "git is required to install agbot"

os=$(uname -s 2>/dev/null || echo unknown)
case "$os" in
    Linux)  log "detected Linux — server role available (agbot up)" ;;
    Darwin) log "detected macOS — farmer app available with Docker Desktop (agbot up)" ;;
    *)      log "unrecognised OS '$os' — proceeding anyway" ;;
esac

# --- fetch / update the repo ----------------------------------------------
mkdir -p "$AGBOT_HOME"
if [ -d "$REPO_DIR/.git" ]; then
    [ -z "$(git -C "$REPO_DIR" status --porcelain)" ] \
        || die "local changes exist in $REPO_DIR; installation stopped to preserve them. Commit or move your changes before retrying."
    log "updating existing checkout in $REPO_DIR"
    git -C "$REPO_DIR" fetch --quiet origin "$AGBOT_REF"
    git -C "$REPO_DIR" merge --ff-only --quiet FETCH_HEAD \
        || die "cannot fast-forward $REPO_DIR to $AGBOT_REF; local history was preserved. Resolve the checkout manually before retrying."
else
    log "cloning $AGBOT_REPO ($AGBOT_REF) into $REPO_DIR"
    git clone --quiet --depth 1 --branch "$AGBOT_REF" "$AGBOT_REPO" "$REPO_DIR"
fi

AGBOT_BIN="$REPO_DIR/scripts/agbot"
[ -f "$AGBOT_BIN" ] || die "agbot CLI not found at $AGBOT_BIN after checkout"
chmod +x "$AGBOT_BIN" 2>/dev/null || true

# --- link onto PATH --------------------------------------------------------
link_target=""
if [ -n "${BIN_DIR:-}" ]; then
    link_target="$BIN_DIR"
elif [ -w /usr/local/bin ] 2>/dev/null; then
    link_target=/usr/local/bin
else
    link_target="$HOME/.local/bin"
fi
mkdir -p "$link_target"

if ln -sf "$AGBOT_BIN" "$link_target/agbot" 2>/dev/null; then
    log "linked agbot -> $link_target/agbot"
else
    die "could not link agbot into $link_target (set BIN_DIR to a writable dir)"
fi

case ":$PATH:" in
    *":$link_target:"*) : ;;
    *) log "NOTE: add $link_target to your PATH (e.g. export PATH=\"$link_target:\$PATH\")" ;;
esac

log "done. Next:"
log "  Start Docker (Docker Desktop on Mac, Docker Engine on Linux)."
log "  agbot up                          # start the farmer app and show your first-login code"
log "  Open http://127.0.0.1:8080/portal  # sign in, then add your farm and fields"
