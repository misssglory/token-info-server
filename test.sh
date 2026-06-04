#!/usr/bin/env bash
set -euo pipefail

HOST="${HOST:-127.0.0.1}"
PORT="${PORT:-9002}"
MINT="${1:-EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v}"

payload="$(
  jq -nc \
    --arg context "$MINT" \
    '{
      context: $context
    }'
)"

echo "$payload" | nc "$HOST" "$PORT"
