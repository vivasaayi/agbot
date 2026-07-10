#!/bin/sh
# install.sh — bootstrap the `agbot` CLI on a Linux server or a Mac.
#
#   curl -fsSL https://raw.githubusercontent.com/vivasaayi/agbot/main/scripts/install.sh | sh
#
# It clones (or updates) the AGBot repo into $AGBOT_HOME (default ~/.agbot/repo)
# and links the `agbot` command onto your PATH. Afterwards:
#
#   Linux server:  agbot up            # pull + run the geo_hub appliance
#   Mac desktop:   agbot viewer        # build + run geo_viewer against a server
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
    Darwin) log "detected macOS — desktop role available (agbot viewer / agbot sim)" ;;
    *)      log "unrecognised OS '$os' — proceeding anyway" ;;
esac

# --- fetch / update the repo ----------------------------------------------
mkdir -p "$AGBOT_HOME"
if [ -d "$REPO_DIR/.git" ]; then
    log "updating existing checkout in $REPO_DIR"
    git -C "$REPO_DIR" fetch --quiet --depth 1 origin "$AGBOT_REF"
    git -C "$REPO_DIR" checkout --quiet "$AGBOT_REF"
    git -C "$REPO_DIR" reset --hard --quiet "origin/$AGBOT_REF" 2>/dev/null || true
else
    log "cloning $AGBOT_REPO ($AGBOT_REF) into $REPO_DIR"
    git clone --quiet --depth 1 --branch "$AGBOT_REF" "$AGBOT_REPO" "$REPO_DIR" \
        || git clone --quiet --depth 1 "$AGBOT_REPO" "$REPO_DIR"
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
if [ "$os" = "Darwin" ]; then
    log "  agbot config set geo_hub_url http://<server>:8080   # point at your server"
    log "  agbot viewer                                        # build + run the desktop viewer"
else
    log "  agbot up                                            # pull + run the geo_hub appliance"
    log "  open http://localhost:8080/portal                   # farmer portal (PWA)"
fi
