# maxmux-autostart.sh — Auto-start/attach maxmux when opening a shell
#
# Source this file from your .zshrc or .bashrc. Do NOT execute it directly.
#
# Usage:
#
#   # Minimal — autostart with defaults:
#   [ -f /path/to/extras/shell/maxmux-autostart.sh ] && \
#     . /path/to/extras/shell/maxmux-autostart.sh
#
#   # Named session:
#   MAXMUX_AUTOSTART_SESSION="main"
#   . /path/to/extras/shell/maxmux-autostart.sh
#
#   # Keep shell alive after detach:
#   MAXMUX_AUTOSTART_EXEC=0
#   . /path/to/extras/shell/maxmux-autostart.sh
#
#   # Disable autostart (e.g. in a subshell):
#   MAXMUX_AUTOSTART=0 zsh
#
# Configuration variables (set BEFORE sourcing):
#
#   MAXMUX_AUTOSTART          — set to 0 to disable entirely
#   MAXMUX_AUTOSTART_EXEC     — 1 (default): exec replaces the shell;
#                                0: shell survives after detach
#   MAXMUX_AUTOSTART_SESSION  — named session to attach/create (empty = default)
#   MAXMUX_AUTOSTART_BINARY   — path to the maxmux binary (default: "maxmux")
#   MAXMUX_AUTOSTART_SKIP_SSH — 1: skip autostart inside SSH sessions (default: 0)
#   MAXMUX_AUTOSTART_SKIP_VSCODE — 0: also start inside VS Code terminal
#                                  (default: 1 = skip)
#   MAXMUX_AUTOSTART_SKIP_IDE — 0: also start inside JetBrains IDE terminals
#                                (default: 1 = skip)

# --- Guard checks ---

# 1. Explicitly disabled
[ "${MAXMUX_AUTOSTART:-}" = "0" ] && return 2>/dev/null

# 2. Already inside a maxmux session
[ -n "${MAXMUX:-}" ] && return 2>/dev/null

# 3. Non-interactive shell
case $- in
  *i*) ;;
  *) return 2>/dev/null ;;
esac

# 4. No TTY on stdin
[ -t 0 ] || return 2>/dev/null

# 5. Another terminal multiplexer is active
[ -n "${TMUX:-}" ] && return 2>/dev/null
[ -n "${STY:-}" ] && return 2>/dev/null

# 6. SSH session (skip if configured)
if [ "${MAXMUX_AUTOSTART_SKIP_SSH:-0}" = "1" ]; then
  [ -n "${SSH_CLIENT:-}" ] && return 2>/dev/null
  [ -n "${SSH_CONNECTION:-}" ] && return 2>/dev/null
  [ -n "${SSH_TTY:-}" ] && return 2>/dev/null
fi

# 7. VS Code terminal (skip by default)
if [ "${MAXMUX_AUTOSTART_SKIP_VSCODE:-1}" = "1" ]; then
  [ "${TERM_PROGRAM:-}" = "vscode" ] && return 2>/dev/null
fi

# 8. JetBrains IDE terminal (skip by default)
if [ "${MAXMUX_AUTOSTART_SKIP_IDE:-1}" = "1" ]; then
  case "${TERMINAL_EMULATOR:-}" in
    *JetBrains*) return 2>/dev/null ;;
  esac
fi

# 9. Binary not found
_maxmux_bin="${MAXMUX_AUTOSTART_BINARY:-maxmux}"
if ! command -v "$_maxmux_bin" >/dev/null 2>&1; then
  unset _maxmux_bin
  return 2>/dev/null
fi

# --- Launch ---

_maxmux_session="${MAXMUX_AUTOSTART_SESSION:-}"
_maxmux_exec="${MAXMUX_AUTOSTART_EXEC:-1}"

if [ -z "$_maxmux_session" ]; then
  # Default mode: bare maxmux starts server + attaches to default session
  if [ "$_maxmux_exec" = "1" ]; then
    _maxmux_run="$_maxmux_bin"
    unset _maxmux_bin _maxmux_session _maxmux_exec
    exec "$_maxmux_run"
  else
    _maxmux_run="$_maxmux_bin"
    unset _maxmux_bin _maxmux_session _maxmux_exec
    "$_maxmux_run"
    unset _maxmux_run
  fi
else
  # Named session: try attach, fall back to new-session
  # Cannot use exec here due to the fallback chain
  _maxmux_run="$_maxmux_bin"
  unset _maxmux_bin
  "$_maxmux_run" attach -t "$_maxmux_session" 2>/dev/null || \
    "$_maxmux_run" new-session -s "$_maxmux_session"

  # In exec mode, exit the shell after maxmux returns (simulates exec behavior)
  if [ "$_maxmux_exec" = "1" ]; then
    unset _maxmux_session _maxmux_exec _maxmux_run
    exit 0
  fi

  unset _maxmux_session _maxmux_exec _maxmux_run
fi
