//! Genesis presets for VOIDKIN. Dev preset uses the well-known Hardhat/Anvil test account
//! so MetaMask and Hardhat work out of the box on a local network. NEVER use on mainnet.
use crate::*;
use alloc::string::String;
use serde_json::Value;
use sp_genesis_builder::{self, PresetId};

pub fn dev_genesis() -> Value {
    // Hardhat account #0
    let alice: AccountId = H160(hex_literal::hex!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"));
    serde_json::json!({
        "balances": { "balances": [[alice, 1_000_000u128 * KIN]] },
        "sudo": { "key": alice },
        // Dev validator = well-known //Alice keys. Replace for any real network.
        "aura": { "authorities": ["5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"] },
        "grandpa": { "authorities": [["5FA9nQDVg267DEd8m1ZypXLBnvN7SFxYwV7ndqSYGiN9TTpu", 1]] },
        "evmChainId": { "chainId": CHAIN_ID },
        "evm": { "accounts": {} },
        "baseFee": { "baseFeePerGas": "0x3b9aca00", "elasticity": 125000 }
    })
}

pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
    let patch = match id.as_ref() {
        sp_genesis_builder::DEV_RUNTIME_PRESET | sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => dev_genesis(),
        _ => return None,
    };
    Some(serde_json::to_string(&patch).expect("serializable").into_bytes())
}

pub fn preset_names() -> Vec<PresetId> {
    vec![PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET)]
}
