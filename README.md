# KCC20 reference

Argent implementation of the KCC20 fungible-token convention for Kaspa, with a public-mint app, offline Rust examples, and contract tests.

## Run

```sh
cargo run --locked --bin kcc20
cargo run --locked --bin kcc20 -- hash-chain
cargo run --locked --bin kcc20 -- mint
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

`rust-toolchain.toml` selects Rust 1.94.1. Compiler, runtime, and consensus revisions are pinned in `Cargo.toml`; `Cargo.lock` pins the dependency graph. All examples compile `KCC20PublicMint` from `contracts/public_mint.ag` into `build/public-mint/`.

The example transfers 11 tokens from Alice into Bob's existing token UTXO. Bob's amount increases from 200 to 211, exceeding his borrow threshold of 10. Alice signs the delegate input and receives 89 tokens as change. Both successor outputs preserve the inputs' 1,000 sompi values.

Keys, outpoints, and the covenant ID are deterministic test data. The example does not connect to a node or submit a transaction.

The `hash-chain` example builds two consecutive borrowed receives. Bob's tokens
increase from 200 to 201 to 202, while Alice's decrease from 100 to 99 to 98.
Each borrow reveals the next guard and signs with its one-time key. The second
transaction spends the actual outputs of the first, advancing the chain to its
terminal guard. Both transactions are validated locally; nothing is submitted.

The `mint` example builds `contracts/public_mint.ag` into `build/public-mint/`.
It launches a minter with an allowance of 25 tokens, a per-mint limit of 10,
and a seeder in the same covenant family, then chooses to mint **10, 10, 5**
tokens. Smaller positive amounts are also allowed.
Each mint spends the previous minter output;
Alice transfers each actual minted token output to Bob. Funding inputs are
synthetic OP_TRUE UTXOs. Genesis, mints, and transfers are executed locally;
nothing is submitted to a network.

## Public mint

`KCC20PublicMint` imports the unchanged `KCC20` actor and adds `PublicMint`
and `TokenSeed`:

```text
PublicMintState {
    remaining:            int
    mint_amount:          int
    owner:                pubkey
    extension_commitment: byte[32]
}
```

`mint(KCC20State recipient_state)` is permissionless.
The caller supplies the recipient's complete token state. Its amount must be
positive and no greater than either `mint_amount` or `remaining`. Each call reduces
the successor minter's allowance by the recipient amount. The per-mint limit,
deposit owner, and extension commitment stay fixed, and calls after exhaustion
fail. The initial allowance is the supply available to that minter, so no
separate cap or minted counter is stored.

The entry creates exactly one minter successor and one KCC20 recipient output.
It preserves the minter's sompi value; the caller funds the new token output and
transaction fee. The recipient's owner and borrow schemes are validated; its
borrow guard is caller-selected. Its extension commitment must match the minter's
commitment, chosen at launch and preserved by every mint. The example selects
disabled borrowing and a commitment of 32 zero bytes. Zero is a convention of this
deployment, with no special KCC20 meaning. The commitment fixes the extension
identity; the minter does not interpret or validate the underlying extended state.
KCC20 interprets the threshold only when the token is borrowed.
The Schnorr `owner` key controls deposit reclaim; minting remains permissionless.

`split(int take, pubkey new_owner)` is also permissionless. The caller chooses a
positive allowance no greater than half of `remaining`. The first successor keeps
the original owner and KAS value; the second gets `take`, the chosen owner, and a
caller-funded, unrestricted KAS value. Both retain the mint cap and extension
commitment. The half limit applies per split.

`reclaim(sig owner_signature)` releases an exhausted minter's deposit, requiring
`remaining == 0`. In `reclaim_into()`, the surviving minter leads and consumes a
retiring minter with the same mint cap and extension commitment. It adds the
retiring minter's allowance while preserving its own owner and KAS. The retiring
input uses `reclaim_delegator(sig owner_signature)` to authorize releasing its
deposit; the survivor needs no signature. Reclaim authorizes ordinary
outputs through the retiring owner's transaction signature; the contract does
not fix a payout address.

A single-minter deployment should start with one `PublicMint`, at least one
`TokenSeed`, and no initial token balances. The advertised supply and initial
seeder presence must be checked against the full genesis output group. All
quantities are integer base units in the KCC1 range.

`KCC20PublicMint` is the reference app, containing `KCC20`, `PublicMint`, and
`TokenSeed`. Argent adds template context and witness arguments when linking
the actors. Use `build/public-mint/artifact.json` to construct its transactions.

## Zero-token receiving UTXOs

`TokenSeed` stores a Schnorr deposit `owner` and a fixed `extension_commitment`.
Its permissionless `create(KCC20State recipient_state)` entry requires amount
zero, supported policy schemes, and the seed's extension commitment. It recreates
the seed with unchanged state and KAS, while the caller chooses and funds the
recipient's KAS deposit. An amount-threshold guard of zero lets that recipient
receive any positive token payment through borrowed receive.

`split(pubkey new_owner)` preserves the first seed's owner and KAS and creates a
second seed with the chosen owner and unrestricted KAS. In `reclaim()`, the
surviving seed leads and consumes a retiring seed with the same extension
commitment, then recreates itself with unchanged state and KAS. The retiring
input uses `reclaim_delegator(sig owner_signature)`; the survivor needs no
signature. There is no reclaim path for a lone seed:
every seed transition leaves at least one seed alive.

Seeds must be included in the token's original genesis family; a new genesis
produces a different covenant ID. They remain available after all minters are
exhausted or reclaimed.

## Reference configuration

- App: `KCC20PublicMint`, containing `KCC20`, `PublicMint`, and `TokenSeed`.
- Token actor: `KCC20`.
- Leader: `transfer(KCC20State[], byte[])`.
- Delegate: `transfer_delegator(byte[])`.
- Transfer bounds: one to three token inputs and one to three token outputs.
- Owner role: `state.owner`, interpreted through `state.owner_scheme`.

The state fields appear in this order:

```text
KCC20State {
    amount:               int
    owner:                byte[32]
    owner_scheme:         byte
    borrow_scheme:        byte
    borrow_guard:         byte[32]
    extension_commitment: byte[32]
}
```

All token amounts are non-negative. A transfer preserves their total and the shared `extension_commitment`. The commitment is opaque; no value has special meaning. Totals must fit the KCC1 integer range, up to `2^63 - 1`.

The leader is the lowest-indexed input in the covenant family. Its witness begins with `0x00` for owner authorization or `0x01` for borrowed receive. Each delegate supplies only its owner-authorization bytes.

### Owner authorization

| Byte | KCC2 scheme | Owner value | Authorization bytes |
| --- | --- | --- | --- |
| `0x00` | `p2pk-schnorr/v1` | Schnorr public key | 65-byte transaction signature |
| `0x01` | `p2pkh-schnorr/v1` | Public-key hash | 32-byte public key, then 65-byte signature |
| `0x02` | `p2pkh-ecdsa/v1` | Public-key hash | 33-byte compressed public key, then 65-byte signature |
| `0x03` | `p2sh/v1` | Redeem-script commitment | One unsigned transaction input index |
| `0x04` | `covenant-id/v1` | Covenant ID | Empty |

Both public-key hashes use unkeyed BLAKE3 over the public-key bytes, without a domain key. P2SH uses the version-0 envelope committing to the exact redeem script. Covenant-ID authorization requires a participating input of that family; KCC2 permits the active input itself to satisfy this check.

Transaction signatures contain 64 signature bytes followed by a consensus sighash byte. The reference accepts the consensus-valid sighash types. The example signs with `SIGHASH_ALL`.

### Borrowed receive

Only the leader can be borrowed. Its successor is the first output in covenant-family order, which need not be transaction output zero. That output preserves its owner, owner scheme, borrow scheme, and extension commitment, increases its token amount, and does not reduce its sompi value.

| Byte | Scheme | Guard | Authorization after the path byte |
| --- | --- | --- | --- |
| `0x00` | `disabled/v1` | Unused | Rejected |
| `0x01` | `amount-threshold/v1` | Signed threshold in the first eight bytes | Empty; increase must exceed the effective threshold |
| `0x02` | `schnorr-signature/v1` | Schnorr public key | 65-byte transaction signature |
| `0x03` | `hash-chain/v1` | Current chain commitment | 32-byte next guard, 32-byte one-time public key, 65-byte transaction signature |

Thresholds use KCC1's eight-byte little-endian signed-magnitude payload. Negative thresholds are treated as zero when borrowing, so borrowing always requires a strictly positive token increase. Normal owner-authorized transfers may create or preserve any threshold payload. The remaining guard bytes do not affect the threshold. Threshold and signature borrows preserve the complete guard.

A hash-chain borrow requires `BLAKE3(next_guard || one_time_public_key) == borrow_guard` and a valid transaction signature by that key. Its successor stores `next_guard`. Link reuse is rejected after advancement; independent UTXOs should use independent chains, and owners can deliberately reset or copy guards through normal transfers.

## Tests and scope

Tests encode transactions directly and execute them in the covenant-enabled VM. This lets malformed cardinalities, role selections, witnesses, and continuations reach the contract without being rejected by a transaction builder first. The suite covers each owner scheme as leader and delegate, all borrow schemes, all supported input/output shapes, authorization failures, conservation, state preservation, and integer boundaries.

ABI regression tests pin state field order and the KCC1 dispatch tags.

Three initial lifecycle tests cover minter split, mint, and allowance return;
exhausted deposit reclaim; and seed split, zero-token creation, borrowed receive,
and reclaim with a surviving seed. They include a few basic rejection checks;
the new entrypoints do not yet have a full conformance suite.

Transfer tests exercise the KCC20 actor in the complete public-mint app.
The public-mint tests cover local genesis, successive issuance and transfer,
caller-selected amounts and borrow guards, fixed extension commitments, exhaustion,
integer boundaries, altered states, output shape, and preservation of
the minter's sompi. These offline tests do not exercise network submission or
wallet synchronization, and are not an independent security audit.

## Files

- `contracts/kcc20.ag`: token state, authorization schemes, and transfer actor.
- `contracts/public_mint.ag`: public-mint app with minter split and reclaim.
- `contracts/token_seed.ag`: zero-token creation, seed split and reclaim actor.
- `src/bin/kcc20/main.rs`: threshold-borrow example.
- `src/bin/kcc20/chain_borrow.rs`: two successive hash-chain borrowed receives.
- `src/bin/kcc20/public_mint.rs`: local launch, mint, and transfer example.
- `src/bin/kcc20/public_mint/tests.rs`: issuance tests.
- `src/bin/kcc20/public_mint/tests/lifecycle.rs`: initial minter and seeder lifecycle tests.
- `src/bin/kcc20/support.rs`: offline keys and transaction signing.
- `src/bin/kcc20/tests.rs`: contract and ABI tests.

The generated artifact and this configuration describe how to construct reference transactions. KCC20, KCC1, and KCC2 define the normative conventions.

The generated SIL, artifact, and manifest for `KCC20PublicMint` are tracked in
`fixtures/public-mint/` so contract and compiler changes can be reviewed in Git.
Temporary build output stays in the ignored `build/` directory. Regenerate the
pinned fixtures with:

```sh
cargo run --locked --example build_contracts
```
