use std::sync::OnceLock;

use argent_runtime::{BuilderError, BuilderResult, execute_transaction_with_covenants};
use kaspa_consensus_core::tx::Transaction;

use super::*;

fn artifact() -> &'static Artifact {
    static ARTIFACT: OnceLock<Artifact> = OnceLock::new();
    ARTIFACT.get_or_init(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        build_file(
            root.join("contracts/public_mint.ag"),
            root.join("build/test-public-mint"),
        )
        .expect("mintable app compiles")
    })
}

fn mint_case(remaining: i64, lot: i64, scheme: u8) -> BuilderResult<(Transaction, Vec<UtxoEntry>)> {
    let builder = TxBuilder::new(artifact())?;
    let covenant_id = Hash::from_bytes([0x11; 32]);
    let (_, owner) = demo_keys(0xaa);
    let before = minter_state(remaining, lot);
    let amount = remaining.min(lot);
    let mut recipient = token_state(&owner, amount);
    recipient.insert("owner_scheme".into(), scheme.into());
    let utxo = builder.covenant_utxo(
        "PublicMint",
        before.clone(),
        TOKEN_OUTPUT_SOMPI,
        1,
        false,
        Some(covenant_id),
    )?;
    let utxos = vec![utxo.clone(), funding()];
    let transaction = builder.build(
        &TxContext::new()
            .actor_input(
                "PublicMint",
                before,
                EntryCall::new("mint").args(args!(owner.to_vec(), scheme)),
                demo_outpoint(1, 0),
                utxo,
                0,
            )
            .input(demo_outpoint(2, 0), funding(), Vec::new(), 0)
            .actor_output(
                "PublicMint",
                minter_state(remaining - amount, lot),
                CovenantBinding::new(0, covenant_id),
                TOKEN_OUTPUT_SOMPI,
            )
            .actor_output(
                "KCC20",
                recipient,
                CovenantBinding::new(0, covenant_id),
                TOKEN_OUTPUT_SOMPI,
            ),
    )?;
    Ok((transaction, utxos))
}

fn rejects(result: BuilderResult<impl Sized>) {
    match result {
        Err(BuilderError::InputScript { .. }) => {}
        Err(error) => panic!("expected mint script rejection, got {error:?}"),
        Ok(_) => panic!("invalid mint accepted"),
    }
}

#[test]
fn launch_mint_and_transfer_actual_outputs() {
    run(artifact()).unwrap();
}

#[test]
fn public_mint_accepts_full_partial_and_maximum_allotments() {
    for (remaining, lot) in [
        (25, 10),
        (10, 10),
        (5, 10),
        (i64::MAX, 1),
        (i64::MAX, i64::MAX),
    ] {
        mint_case(remaining, lot, OWNER_P2PK_SCHNORR).unwrap();
    }
    for scheme in 0..=4 {
        mint_case(25, 10, scheme).unwrap();
    }
}

#[test]
fn public_mint_rejects_exhaustion_invalid_allowances_and_owner_schemes() {
    for (remaining, lot) in [(0, 10), (-1, 10), (25, 0), (25, -1)] {
        rejects(mint_case(remaining, lot, OWNER_P2PK_SCHNORR));
    }
    rejects(mint_case(25, 10, 5));
}

#[test]
fn mint_binds_allowance_policy_and_recipient_state() {
    let builder = TxBuilder::new(artifact()).unwrap();
    let (_, owner) = demo_keys(0xaa);
    let cases: [(usize, &str, &str, ArtifactValue); 8] = [
        (0, "PublicMint", "remaining", 16i64.into()),
        (0, "PublicMint", "mint_amount", 11i64.into()),
        (1, "KCC20", "amount", 11i64.into()),
        (1, "KCC20", "owner", vec![0xbbu8; 32].into()),
        (1, "KCC20", "owner_scheme", 1u8.into()),
        (1, "KCC20", "borrow_scheme", 1u8.into()),
        (1, "KCC20", "borrow_guard", vec![1u8; 32].into()),
        (1, "KCC20", "extension_commitment", vec![1u8; 32].into()),
    ];
    for (index, actor, field, value) in cases {
        let (mut tx, utxos) = mint_case(25, 10, OWNER_P2PK_SCHNORR).unwrap();
        let mut state = if index == 0 {
            minter_state(15, 10)
        } else {
            token_state(&owner, 10)
        };
        state.insert(field.into(), value);
        tx.outputs[index].script_public_key = builder
            .genesis_output(actor, state, TOKEN_OUTPUT_SOMPI)
            .unwrap()
            .script_public_key;
        // Mutate the constructed transaction so rejection comes from the VM.
        rejects(execute_transaction_with_covenants(&mut tx, utxos));
    }
}

#[test]
fn mint_rejects_drained_principal_and_wrong_covenant() {
    let (mut tx, utxos) = mint_case(25, 10, OWNER_P2PK_SCHNORR).unwrap();
    tx.outputs[0].value -= 1;
    rejects(execute_transaction_with_covenants(&mut tx, utxos));

    let (mut tx, utxos) = mint_case(25, 10, OWNER_P2PK_SCHNORR).unwrap();
    tx.outputs[1].covenant = Some(CovenantBinding::new(0, Hash::from_bytes([0x22; 32])));
    // The covenant binding is rejected before script execution.
    assert!(matches!(
        execute_transaction_with_covenants(&mut tx, utxos),
        Err(BuilderError::TxScript(_))
    ));
}

#[test]
fn mint_rejects_missing_or_extra_outputs_and_multiple_minters() {
    let (mut tx, utxos) = mint_case(25, 10, OWNER_P2PK_SCHNORR).unwrap();
    tx.outputs.swap(0, 1);
    rejects(execute_transaction_with_covenants(&mut tx, utxos));

    for index in 0..2 {
        let (mut tx, utxos) = mint_case(25, 10, OWNER_P2PK_SCHNORR).unwrap();
        tx.outputs.remove(index);
        rejects(execute_transaction_with_covenants(&mut tx, utxos));

        let (mut tx, utxos) = mint_case(25, 10, OWNER_P2PK_SCHNORR).unwrap();
        tx.outputs.push(tx.outputs[index].clone());
        rejects(execute_transaction_with_covenants(&mut tx, utxos));
    }
    let (mut tx, mut utxos) = mint_case(25, 10, OWNER_P2PK_SCHNORR).unwrap();
    let mut second = tx.inputs[0].clone();
    second.previous_outpoint = demo_outpoint(3, 0);
    tx.inputs.push(second);
    utxos.push(utxos[0].clone());
    rejects(execute_transaction_with_covenants(&mut tx, utxos));
}
