# Contract source lookup extension

## Requirements
* Existing PostgreSQL DB as configured by token-info-server. A separate `token_contract_sources` table is created automatically.
* `ETHERSCAN_API_KEY` enables Etherscan V2 source lookup on Ethereum, BSC, and Robinhood. Robinhood additionally uses the scanner's HTML fallback when V2 is unavailable.
* RPC metadata lookup still uses `reqwest 0.13` with rustls.

## Usage

```bash
nix develop
cargo build --release
./test.sh --chain robinhood 0xYOURADDRESS | jq .
./test.sh --chain robinhood --sources 0xYOURADDRESS | jq '.sources'
./test.sh --chain robinhood --no-cache --sources 0xYOURADDRESS | jq .
```

Raw TCP protocol, port 9002:

```bash
printf '%s\n' '{"context":"0xYOURADDRESS","chain":"robinhood"}' | nc 127.0.0.1 9002
printf '%s\n' '{"type":"sources","address":"0xYOURADDRESS","chain":"robinhood"}' | nc 127.0.0.1 9002
```

`launchpad.matches` provides evidence flags, not proof of the original launch platform. Detection checks verified contract names, filenames, and source text. Patterns include Pons, long.xyz, Pangea, and scanner's original entities.

Source fetches use asynchronous Reqwest requests and zstd level 6 in Tokio blocking workers. Sources are cached without TTL. Existing metadata cache is checked first and retains its original TTL.

**Limitations**: This archive was not compiled in the current environment (Rust/Cargo/Nix unavailable). It also does not include `Cargo.lock`; retain your locally generated lockfile, then update the Nix `cargoHash` after dependencies change. The Etherscan / explorer APIs may rate limit and unverified contracts may have no source. Large raw-source responses can be substantial.

## NixOS installation

1. Compile locally and run `nix build -L`; if prompted for the vendor hash, set the `got:` SHA-256 in repository `flake.nix`.
2. Commit and push to `github:misssglory/token-info-server` (ensure `.env` is not staged).
3. `cd ~/nixos-dotfiles && nix flake update token-info-server && git add flake.lock && sudo nixos-rebuild switch --flake .#nixos-btw -L`.
4. Restart/check: `sudo systemctl restart token-info-server; journalctl -fu token-info-server`.
5. Set `ETHERSCAN_API_KEY` using systemd `EnvironmentFile` such as root-owned `/etc/token-info-server.env`, not a public flake value.
