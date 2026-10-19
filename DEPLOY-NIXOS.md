# Token Info Server — NixOS deployment

## Local test

```sh
nix develop
./test.sh --chain robinhood 0x710a0414dc599112987515ac8d76e0cd28546bc1 | jq .
```

Logging now goes to stderr, JSON to stdout. `test.sh` disables outgoing TCP forwarding but ENABLES persistent PostgreSQL caching by default. Use `--no-cache` to bypass it.

## Nix package build

The supplied ZIP did not contain Cargo.lock. Keep your existing tracked Cargo.lock in the working checkout. The new Rustls feature affects Cargo vendoring. `flake.nix` intentionally uses fakeHash until you calculate its replacement. Run `nix build -L`, take the `got: sha256-...` from the fixed-output hash mismatch, paste into `cargoHash` in flake.nix, then run `nix build -L` again.

Commit and push source changes (including Cargo.lock if tracked), without .env.

## Deploy latest GitHub source on host

```sh
cd ~/nixos-dotfiles
nix flake update token-info-server
git add flake.lock
sudo nixos-rebuild switch --flake .#nixos-btw -L
systemctl status token-info-server
journalctl -fu token-info-server
```

The existing bundle.nix, rex.nix, sops.nix stay unchanged.

## Stop production and test manually

```sh
sudo systemctl stop token-info-server
cd ~/gh/token-info-server
nix develop -c ./test.sh --chain robinhood 0x710a0414dc599112987515ac8d76e0cd28546bc1
sudo systemctl start token-info-server
```

## TCP lookup (not HTTP)

Start the server without outgoing forwarding on port 9002:

```sh
TCP_ENDPOINT_ENABLED=false ./result/bin/token-info-server --config /dev/null --host 127.0.0.1 --port 9002
```

Then send one newline-terminated request (read one newline-terminated JSON response):

```sh
printf '%s\n' '{"context":"0x710a0414dc599112987515ac8d76e0cd28546bc1","chain":"robinhood"}' | nc 127.0.0.1 9002
```

Raw TCP isn't HTTP; don't use curl against it. Remote access should use SSH tunneling (`ssh -L 9002:127.0.0.1:9002 mg@YOUR_HOST`) or a VPN rather than exposing an unauthenticated endpoint.

## Cache-first behavior (CLI and TCP)

The database must exist and credentials must work. Configure `DB_*` variables using
`--config /dev/null` or provide an appropriate TOML configuration.
`test.sh` uses `--config /dev/null` and PostgreSQL by default.

```sh
./test.sh --verbose --chain robinhood 0x710a0414dc599112987515ac8d76e0cd28546bc1 | jq .
./test.sh --verbose --chain robinhood 0x710a0414dc599112987515ac8d76e0cd28546bc1 | jq .
```

The second request should log `Cache hit ... (no RPC)` to stderr.
Automatic chain detection checks all candidate-chain caches before probing any RPC.
Expired records count as misses, and unrecognized tokens are not negatively cached.
Database read failures return an error rather than invoking RPC.

For manual TCP server:
```sh
TCP_ENDPOINT_ENABLED=false cargo run --release -- --config /dev/null --host 127.0.0.1 --port 9002
```
Use `printf '%s\n' '{"context":"0x710a0414dc599112987515ac8d76e0cd28546bc1","chain":"robinhood"}' | nc 127.0.0.1 9002` from another terminal; read a single response line.

When deploying to NixOS, make sure the service's `DB_*` settings and environment file point at the SAME database used for manual tests. In `modules/token-info-server.nix`, avoid `--no-cache`; keep `--config /dev/null` if using environment-based configuration. `bundle.nix`, `rex.nix`, and `sops.nix` remain untouched.
