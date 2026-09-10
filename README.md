# KCC20 reference

Argent implementation of the KCC20 fungible-token convention for Kaspa, with an offline Rust example and contract tests.

## Run

```sh
cargo run --locked --bin kcc20
cargo run --locked --bin kcc20 -- hash-chain
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

`rust-toolchain.toml` selects Rust 1.94.1. Compiler, runtime, and consensus revisions are pinned in `Cargo.toml`; `Cargo.lock` pins the dependency graph. The example writes its compiled artifact to `build/artifact.json`.

The example transfers 11 tokens from Alice into Bob's existing token UTXO. Bob's amount increases from 200 to 211, exceeding his borrow threshold of 10. Alice signs the delegate input and receives 89 tokens as change. Both successor outputs preserve the inputs' 1,000 sompi values.

Keys, outpoints, and the covenant ID are deterministic test data. The example does not connect to a node or submit a transaction.

The `hash-chain` example builds two consecutive borrowed receives. Bob's tokens
increase from 200 to 201 to 202, while Alice's decrease from 100 to 99 to 98.
Each borrow reveals the next guard and signs with its one-time key. The second
transaction spends the actual outputs of the first, advancing the chain to its
terminal guard. Both transactions are validated locally; nothing is submitted.

## Reference configuration

- Actor: `KCC20` in app `KCC20Reference`.
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

Both public-key hashes use KCC2's keyed BLAKE3 `PublicKeyHash` domain. P2SH uses the version-0 envelope committing to the exact redeem script. Covenant-ID authorization requires a participating input of that family; KCC2 permits the active input itself to satisfy this check.

Transaction signatures contain 64 signature bytes followed by a consensus sighash byte. The reference accepts the consensus-valid sighash types. The example signs with `SIGHASH_ALL`.

### Borrowed receive

Only the leader can be borrowed. Its successor is the first output in covenant-family order, which need not be transaction output zero. That output preserves its owner, owner scheme, borrow scheme, and extension commitment, increases its token amount, and does not reduce its sompi value.

| Byte | Scheme | Guard | Authorization after the path byte |
| --- | --- | --- | --- |
| `0x00` | `disabled/v1` | Unused | Rejected |
| `0x01` | `amount-threshold/v1` | Non-negative threshold in the first eight bytes | Empty; increase must exceed the threshold |
| `0x02` | `schnorr-signature/v1` | Schnorr public key | 65-byte transaction signature |
| `0x03` | `hash-chain/v1` | Current chain commitment | 32-byte next guard, 32-byte one-time public key, 65-byte transaction signature |

Thresholds use KCC1's eight-byte little-endian signed-magnitude payload. The remaining guard bytes do not affect the threshold. Threshold and signature borrows preserve the complete guard.

A hash-chain borrow requires `BLAKE3(next_guard || one_time_public_key) == borrow_guard` and a valid transaction signature by that key. Its successor stores `next_guard`. Link reuse is rejected after advancement; independent UTXOs should use independent chains, and owners can deliberately reset or copy guards through normal transfers.

## Tests and scope

Tests encode transactions directly and execute them in the covenant-enabled VM. This lets malformed cardinalities, role selections, witnesses, and continuations reach the contract without being rejected by a transaction builder first. The suite covers each owner scheme as leader and delegate, all borrow schemes, all supported input/output shapes, authorization failures, conservation, state preservation, and integer boundaries.

ABI regression tests pin state field order and the KCC1 dispatch tags. CI runs formatting checks, tests, Clippy, and the offline example.

The contract implements transfers within an established token family. Issuance must establish the initial supply and valid states under the reference program. Synthetic UTXO tests do not exercise genesis, network submission, or wallet synchronization, and are not an independent security audit.

## Files

- `contracts/kcc20.ag`: canonical contract.
- `src/bin/kcc20/main.rs`: threshold-borrow example.
- `src/bin/kcc20/chain_borrow.rs`: two successive hash-chain borrowed receives.
- `src/bin/kcc20/support.rs`: offline keys and transaction signing.
- `src/bin/kcc20/tests.rs`: contract and ABI tests.

The generated artifact and this configuration describe how to construct reference transactions. KCC20, KCC1, and KCC2 define the normative conventions.
