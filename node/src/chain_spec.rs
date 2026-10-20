use sc_service::ChainType;
use voidkin_runtime::WASM_BINARY;

pub type ChainSpec = sc_service::GenericChainSpec;

fn properties() -> sc_service::Properties {
    let mut p = sc_service::Properties::new();
    p.insert("tokenSymbol".into(), "KIN".into());
    p.insert("tokenDecimals".into(), 18.into());
    p.insert("ss58Format".into(), 42.into());
    p
}

pub fn development_config() -> Result<ChainSpec, String> {
    Ok(ChainSpec::builder(WASM_BINARY.ok_or("Development wasm not available")?, None)
        .with_name("VOIDKIN Development")
        .with_id("voidkin_dev")
        .with_chain_type(ChainType::Development)
        .with_properties(properties())
        .with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
        .build())
}

pub fn local_testnet_config() -> Result<ChainSpec, String> {
    Ok(ChainSpec::builder(WASM_BINARY.ok_or("Development wasm not available")?, None)
        .with_name("VOIDKIN Local Testnet")
        .with_id("voidkin_local")
        .with_chain_type(ChainType::Local)
        .with_properties(properties())
        .with_genesis_config_preset_name(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET)
        .build())
}
