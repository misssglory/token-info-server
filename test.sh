#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  ./test.sh TOKEN_ADDRESS [CHAIN]
  ./test.sh --chain CHAIN TOKEN_ADDRESS
  ./test.sh --chain auto TOKEN_ADDRESS

Chains: auto (default), solana, ethereum, bsc, robinhood

Options:
  -c, --chain CHAIN     Override automatic chain detection
  --cache               Use PostgreSQL (default)
  --no-cache            Bypass PostgreSQL and query providers directly
  -v, --verbose         Enable verbose application logging
  -h, --help            Show usage

Examples:
  ./test.sh 0x061732dc141e0a2d37f5fa91cf5343f24064e60f robinhood
  ./test.sh --chain robinhood 0x061732dc141e0a2d37f5fa91cf5343f24064e60f
  ./test.sh 0x061732dc141e0a2d37f5fa91cf5343f24064e60f
  ROBINHOOD_RPC=https://your-rpc.example ./test.sh --chain robinhood 0x...

Uses Cargo from the current shell. The TCP forwarder is disabled for local tests.
EOF
}

chain=auto
address=''
use_cache=true
verbose=false
while (($#)); do
  case "$1" in
    -c|--chain)
      if (($# < 2)); then echo 'Missing chain after --chain' >&2; exit 2; fi
      chain="${2,,}"; shift 2 ;;
    --chain=*) chain="${1#*=}"; chain="${chain,,}"; shift ;;
    --cache) use_cache=true; shift ;;
    --no-cache) use_cache=false; shift ;;
    -v|--verbose) verbose=true; shift ;;
    -h|--help) usage; exit 0 ;;
    --) shift; break ;;
    -*) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    *)
      if [[ -z "$address" ]]; then
        address="$1"
      elif [[ "$chain" == auto ]]; then
        chain="${1,,}"
      else
        echo "Unexpected extra argument: $1" >&2; exit 2
      fi
      shift ;;
  esac
done

if [[ -z "$address" ]]; then usage >&2; exit 2; fi
case "$chain" in auto|solana|ethereum|bsc|robinhood) ;; *)
  echo "Unsupported chain: $chain" >&2; exit 2 ;; esac

if [[ "$chain" == solana ]]; then
  if [[ ! "$address" =~ ^[1-9A-HJ-NP-Za-km-z]{32,44}$ ]]; then
    echo "Invalid Solana address: $address" >&2; exit 2
  fi
elif [[ "$chain" == ethereum || "$chain" == bsc || "$chain" == robinhood ]]; then
  if [[ ! "$address" =~ ^0x[[:xdigit:]]{40}$ ]]; then
    echo "Invalid EVM contract address: $address" >&2; exit 2
  fi
fi

cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")"
command -v cargo >/dev/null || { echo 'Cargo not found. Run nix develop first.' >&2; exit 127; }

args=(--config /dev/null)
if [[ "$use_cache" != true ]]; then args+=(--no-cache); fi
if [[ "$verbose" == true ]]; then args+=(--verbose); fi
if [[ "$chain" != auto ]]; then args+=(--chain "$chain"); fi
args+=("$address")

# CLI testing should not try to connect to the TCP forwarder by default.
export TCP_ENDPOINT_ENABLED=false
exec cargo run --quiet --release -- "${args[@]}"
