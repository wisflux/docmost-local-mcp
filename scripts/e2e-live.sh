#!/usr/bin/env bash
#
# Run the live end-to-end test against a REAL Docmost instance.
#
# This creates a real page on your server. Credentials are read from the environment
# (or a local, git-ignored .env.e2e file) and are never committed.
#
# Usage:
#   1. Copy the template and fill it in:
#        cp scripts/e2e-live.env.example .env.e2e
#        $EDITOR .env.e2e
#   2. Run:
#        ./scripts/e2e-live.sh
#
# Or pass everything inline without a file:
#   DOCMOST_BASE_URL=https://docs.example.com DOCMOST_EMAIL=you@example.com \
#   DOCMOST_PASSWORD=secret ./scripts/e2e-live.sh
#
set -euo pipefail

cd "$(dirname "$0")/.."

# Load .env.e2e if present (KEY=VALUE lines; '#' comments allowed).
# Parsed literally (NOT `source`d) so values with shell metacharacters such as
# ), (, @, $, spaces, etc. in a password are taken verbatim.
if [[ -f .env.e2e ]]; then
  echo "[e2e] loading .env.e2e"
  while IFS= read -r line || [[ -n "$line" ]]; do
    # Strip a trailing CR (CRLF files), skip blanks and comments.
    line="${line%$'\r'}"
    [[ -z "$line" || "$line" == \#* ]] && continue
    [[ "$line" != *=* ]] && continue
    key="${line%%=*}"
    value="${line#*=}"
    key="${key//[[:space:]]/}"
    [[ -z "$key" ]] && continue
    # Trim surrounding whitespace from the value (common accidental trailing space).
    value="${value#"${value%%[![:space:]]*}"}"
    value="${value%"${value##*[![:space:]]}"}"
    # Honor one layer of surrounding quotes (use quotes if a value needs edge spaces).
    if [[ ${#value} -ge 2 && "${value:0:1}" == '"' && "${value: -1}" == '"' ]]; then
      value="${value:1:${#value}-2}"
    elif [[ ${#value} -ge 2 && "${value:0:1}" == "'" && "${value: -1}" == "'" ]]; then
      value="${value:1:${#value}-2}"
    fi
    export "$key=$value"
  done < .env.e2e
fi

missing=()
for var in DOCMOST_BASE_URL DOCMOST_EMAIL DOCMOST_PASSWORD; do
  if [[ -z "${!var:-}" ]]; then
    missing+=("$var")
  fi
done
if (( ${#missing[@]} > 0 )); then
  echo "[e2e] ERROR: missing required env var(s): ${missing[*]}" >&2
  echo "[e2e] Set them in your shell or in .env.e2e (see scripts/e2e-live.env.example)." >&2
  exit 1
fi

echo "[e2e] target: $DOCMOST_BASE_URL  user: $DOCMOST_EMAIL"
echo "[e2e] NOTE: this creates (and updates) a real page on the server above."

# --no-default-features avoids the GTK/WebKitGTK system deps the native webview needs;
# login here is headless (email/password), so the webview is not used.
exec cargo test --test live_e2e_test --no-default-features -- --ignored --nocapture
