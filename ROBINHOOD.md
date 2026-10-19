# Robinhood Chain support

Robinhood mainnet chain ID: 4663. Default RPC: https://rpc.mainnet.chain.robinhood.com

## Automatic chain resolution

- Solana base58 addresses use the Solana providers.
- EVM `0x` addresses are probed on Robinhood, Ethereum and BSC, in that order.
- Providers use `eth_getCode` followed by ERC-20 `symbol()`, `name()` and `decimals()` calls.
- An address can exist on multiple EVM networks; automatic detection chooses the first successful network in that order. Set `chain` explicitly when ambiguity is possible.
- Cache entries are scoped by chain; cached nameless results are not considered a valid match.

## Commands

```bash
nix develop
cargo test
cargo run -- --no-cache --chain robinhood 0xYOUR_TOKEN_CA
cargo run -- --no-cache 0xYOUR_TOKEN_CA
```

For TCP lookups (one JSON object per line):

```json
{"context":"0xYOUR_TOKEN_CA","chain":"robinhood"}
```

For automatic resolution, omit `chain`:

```json
{"context":"0xYOUR_TOKEN_CA"}
```

## Configuration

`ROBINHOOD_RPC` sets the RPC URL for runtime environment configuration (the default NixOS service uses this mode). `config.toml` also includes an `[rpc].robinhood` setting; the general EVM providers in this version are initialized from environment settings, consistent with the original repo.

The included archive does not contain `Cargo.lock` (same as the provided source). Generate and commit a lockfile before packaging with Nix. If `cargoHash` is fixed in `flake.nix`, changes to Cargo dependencies will require recalculating it.
