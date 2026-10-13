use argent::genesis::{ArgentCovenantBootstrap, ArgentGenesisPackage, GenesisProofPackage};
use argent_runtime::{CovenantOutput, EntryCall, TxBuilder, TxContext, args};
use kaspa_consensus_core::tx::{CovenantBinding, ScriptPublicKey, UtxoEntry};
use kast::{
    GENESIS_GROUP, MINTER_VALUE, Result, SEED_VALUE, SOMPI_PER_KAS, backed_value, compile,
    minter_state, seeder_state, token_state,
};
use kcc20_reference::demo_support::{demo_keys, demo_outpoint, sign_input};
use std::{fs, path::PathBuf};

fn ordinary(value: u64) -> UtxoEntry {
    UtxoEntry::new(
        value,
        ScriptPublicKey::from_vec(0, vec![0x51]),
        1,
        false,
        None,
    )
}
fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some(command @ ("build" | "fixtures")) if args.next().is_none() => {
            let output = root.join(if command == "fixtures" {
                "fixtures"
            } else {
                "build/kast"
            });
            compile(&output)?;
            println!("compiled KAST contracts into {}", output.display());
        }
        Some("demo") if args.next().is_none() => demo()?,
        _ => {
            return Err("usage: kast build | fixtures | demo".into());
        }
    }
    Ok(())
}

fn demo() -> Result<()> {
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("build/kast-demo");
    fs::create_dir_all(&output)?;
    let compiled = compile(output.join("contracts"))?;
    let bundle = compiled.runtime_bundle()?;
    let builder = TxBuilder::from_bundle(&bundle)?;
    let (alice, owner) = demo_keys(0xaa);
    let (bob_key, bob) = demo_keys(0xbb);
    let script = ScriptPublicKey::from_vec(0, vec![0x51]);
    let context = TxContext::new()
        .input(
            demo_outpoint(1, 0),
            ordinary(4 * MINTER_VALUE + SEED_VALUE),
            Vec::new(),
            0,
        )
        .actor_genesis_output(
            0,
            GENESIS_GROUP,
            "PublicMint",
            minter_state(&owner),
            MINTER_VALUE,
        )
        .actor_genesis_output(
            0,
            GENESIS_GROUP,
            "TokenSeed",
            seeder_state(&owner),
            SEED_VALUE,
        )
        .output(script.clone(), None, 3 * MINTER_VALUE - MINTER_VALUE / 10);
    let bootstrap = ArgentCovenantBootstrap::from_context(&bundle, &context, 0, GENESIS_GROUP)?;
    let genesis = builder.build(&context)?;
    let minter = CovenantOutput::from_tx(&genesis, 0)?;
    let seeder = CovenantOutput::from_tx(&genesis, 1)?;
    let id = minter.covenant_id;
    let argent = GenesisProofPackage::new(ArgentGenesisPackage::new(
        &bundle,
        bootstrap.clone().compose(&bundle)?,
    ));
    argent.verify_argent(id)?;
    fs::write(
        output.join("genesis.json"),
        serde_json::to_vec_pretty(&genesis)?,
    )?;
    fs::write(
        output.join("bootstrap.json"),
        serde_json::to_vec_pretty(&bootstrap)?,
    )?;
    fs::write(output.join("argent-genesis.json"), argent.to_json()?)?;
    let binding = CovenantBinding::new(0, id);
    let seed_value = SEED_VALUE / 2;
    let seed_rebalanced = builder.build(
        &TxContext::new()
            .actor_input(
                "TokenSeed",
                seeder_state(&owner),
                EntryCall::new("rebalance_value")
                    .args_with(move |tx, index| args!(sign_input(tx, index, &alice))),
                seeder.outpoint,
                seeder.utxo,
                0,
            )
            .actor_output("TokenSeed", seeder_state(&owner), binding, seed_value)
            .output(script.clone(), None, SEED_VALUE - seed_value),
    )?;
    let seeder = CovenantOutput::from_tx(&seed_rebalanced, 0)?;
    let mut receiving_state = token_state(&bob, 0);
    receiving_state.insert("borrow_scheme".into(), 1u8.into());
    let seeded = builder.build(
        &TxContext::new()
            .actor_input(
                "TokenSeed",
                seeder_state(&owner),
                EntryCall::new("create").args(args!(receiving_state.clone())),
                seeder.outpoint,
                seeder.utxo,
                0,
            )
            .actor_output("TokenSeed", seeder_state(&owner), binding, seed_value)
            .actor_output("KAST", receiving_state.clone(), binding, 0),
    )?;
    let receiver = CovenantOutput::from_tx(&seeded, 1)?;
    // Permissionless splitting enables two independently spendable mint lanes.
    let split = builder.build(
        &TxContext::new()
            .actor_input(
                "PublicMint",
                minter_state(&owner),
                EntryCall::new("split").args(args!(bob.to_vec())),
                minter.outpoint,
                minter.utxo,
                0,
            )
            .input(demo_outpoint(2, 0), ordinary(MINTER_VALUE), Vec::new(), 0)
            .actor_output("PublicMint", minter_state(&owner), binding, MINTER_VALUE)
            .actor_output("PublicMint", minter_state(&bob), binding, MINTER_VALUE),
    )?;
    let minter = CovenantOutput::from_tx(&split, 1)?;
    let minter_value = MINTER_VALUE / 2;
    let rebalanced = builder.build(
        &TxContext::new()
            .actor_input(
                "PublicMint",
                minter_state(&bob),
                EntryCall::new("rebalance_value")
                    .args_with(move |tx, index| args!(sign_input(tx, index, &bob_key))),
                minter.outpoint,
                minter.utxo,
                0,
            )
            .actor_output("PublicMint", minter_state(&bob), binding, minter_value)
            .output(script.clone(), None, MINTER_VALUE - minter_value),
    )?;
    let minter = CovenantOutput::from_tx(&rebalanced, 0)?;
    let amount = SOMPI_PER_KAS;
    let minted_state = token_state(&owner, amount);
    let mint = builder.build(
        &TxContext::new()
            .actor_input(
                "PublicMint",
                minter_state(&bob),
                EntryCall::new("mint").args(args!(minted_state.clone())),
                minter.outpoint,
                minter.utxo,
                0,
            )
            .input(
                demo_outpoint(3, 0),
                ordinary(backed_value(amount)?),
                Vec::new(),
                0,
            )
            .actor_output("PublicMint", minter_state(&bob), binding, minter_value)
            .actor_output("KAST", minted_state.clone(), binding, backed_value(amount)?),
    )?;
    let token = CovenantOutput::from_tx(&mint, 1)?;
    let mut sent_state = receiving_state.clone();
    sent_state.insert("amount".into(), amount.into());
    let transfer = builder.build(
        &TxContext::new()
            .actor_input(
                "KAST",
                receiving_state,
                EntryCall::new("transfer").args(args!(vec![sent_state.clone()], vec![1u8])),
                receiver.outpoint,
                receiver.utxo,
                0,
            )
            .actor_input(
                "KAST",
                minted_state,
                EntryCall::new("transfer_delegator")
                    .args_with(move |tx, index| args!(sign_input(tx, index, &alice))),
                token.outpoint,
                token.utxo,
                0,
            )
            .actor_output("KAST", sent_state.clone(), binding, backed_value(amount)?),
    )?;
    let token = CovenantOutput::from_tx(&transfer, 0)?;
    let burned = builder.build(
        &TxContext::new()
            .actor_input(
                "KAST",
                sent_state,
                EntryCall::new("burn")
                    .args_with(move |tx, index| args!(sign_input(tx, index, &bob_key))),
                token.outpoint,
                token.utxo,
                0,
            )
            .output(script, None, backed_value(amount)?),
    )?;
    for (name, tx) in [
        ("seed_rebalance_value", seed_rebalanced),
        ("seed_create", seeded),
        ("split", split),
        ("rebalance_value", rebalanced),
        ("mint", mint),
        ("transfer", transfer),
        ("burn", burned),
    ] {
        fs::write(
            output.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&tx)?,
        )?;
    }
    println!("KAS Token (KAST), 8 decimals; synthetic offline demo completed");
    println!("covenant ID: {id}");
    println!(
        "rebalanced seed, created receiver, split and rebalanced minter, minted and transferred 1 KAST, burned for 1 KAS"
    );
    println!(
        "genesis proof: {}",
        output.join("argent-genesis.json").display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_demo() -> Result<()> {
        demo()
    }
}
