#!/usr/bin/env bash
set -euo pipefail

HOST="${HOST:-127.0.0.1}"
PORT="${PORT:-9002}"
MINT="${1:-EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v}"
SYMBOL="${2:-USDC}"

payload="$(
  jq -nc \
    --arg mint "$MINT" \
    --arg symbol "$SYMBOL" \
    --arg channel "Token name search" \
    --arg context "$MINT" \
    --arg text "search $MINT $SYMBOL" \
    --argjson ts "$(date +%s)000000" \
    '{
      command: "tg",
      args: {
        channel: $channel,
        ts: $ts,
        text: $text
      },
      context: $context
    }'
)"

printf '%s\n' "$payload" | nc "$HOST" "$PORT"
