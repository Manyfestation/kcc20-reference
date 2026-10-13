# KAS Token (KAST)

KAST is a KAS-backed KCC20 token written in Argent. It uses eight decimals:
one base unit is backed by one sompi, so one KAST is backed by one KAS.
The example is a separate Rust crate that runs locally with synthetic funding
and reuses the reference's Rust demo helpers through a path dependency.

## Contracts

[`token.ag`](contracts/token.ag) imports the reference's
[KCC20 state and authorization](../../contracts/kcc20.ag). Minting and transfers
enforce exact backing on every token output:

```text
value in sompi = amount in token base units
```

Transfers use the KCC20 owner and borrow rules. Minting sets the extension
commitment to zero; transfers preserve it. `burn` consumes the entire token UTXO
with owner authorization and releases its backing KAS. The owner chooses the
payout outputs and may use some KAS for fees. To burn part of a balance,
split it first; to burn multiple inputs together, merge them first. Borrow
permission does not authorize burning.

[`minter.ag`](contracts/minter.ag) defines an uncapped public minter
with no supply counter or per-mint limit. Callers fund the token backing;
minting preserves the minter's owner and operational KAS. Anyone can
`split(new_owner)` to create another minter funded by the caller.
`rebalance_value(owner_signature)` lets the owner increase or decrease its
operational KAS, preserving the entire minter state and recreating one minter
with unrestricted value. `reclaim()`
retires one minter with its owner's signature while preserving another.

[`token_seed.ag`](contracts/token_seed.ag) creates zero-token receiving UTXOs
with zero sompi, preserving exact backing. Callers choose the recipient's owner
and borrow policy. The seed's state holds only its deposit owner; KAST's extension
commitment is fixed to zero. Seeds can split permissionlessly, rebalance their
operational KAS with an owner signature, and reclaim a retiring seed through its
owner-authorized delegate while preserving another seed.

[`app.ag`](contracts/app.ag) links `PublicMint`, `TokenSeed`, and `KAST`.
Genesis creates one minter, one seed, and no pre-minted tokens. There is no extra
token deposit; output validity follows consensus rules.

## Run locally

From `examples/kast/`:

```sh
cargo run --locked -- build
cargo run --locked -- demo
```

The demo creates the token genesis, rebalances its seed, and creates Bob's empty
receiving UTXO. It splits and rebalances its minter, mints one KAST to Alice,
transfers it into Bob's receiving UTXO through borrowed receive, and burns it for
one KAS. It uses deterministic test keys and submits no transactions to a node.

`build/kast-demo/` contains the genesis transaction (`genesis.json`), initial
state (`bootstrap.json`), Argent genesis proof (`argent-genesis.json`), compiled
contracts, and the seed_rebalance_value, seed_create, split, rebalance_value,
mint, transfer, and burn transactions.

## Debug the demo

The `run_demo` test calls the full offline demo directly, including genesis proof
creation and verification, without requiring command-line arguments:

```sh
cargo test --locked --bin kast tests::run_demo -- --exact --nocapture
```

In VS Code with CodeLLDB installed, click **Debug** above `run_demo` in
`src/main.rs`.
Set breakpoints inside `demo()`, especially at `bootstrap.clone().compose(&bundle)`
and `argent.verify_argent(id)`, to inspect proof construction and verification.
The test writes the same files under `build/kast-demo/` as the CLI demo.

## Fixtures and checks

```sh
cargo run --locked -- fixtures
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

Fixture regeneration updates only KAST artifacts.
[`fixtures`](fixtures/) contains the token descriptor, compiled
artifact, manifest, and SIL. Tests cover backing, owner and borrow authorization,
minter and seed lifecycles, deposit rebalancing, and burning.
