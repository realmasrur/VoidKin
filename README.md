# VOIDKIN: EVM-compatible Layer 1 (token: KIN)

Stack: Substrate (Rust) + Frontier (Ethereum JSON-RPC, MetaMask, Hardhat, ethers.js).
Consensus at launch: Aura (6s blocks) + GRANDPA finality, PoA validator set.

## Status
- [x] Workspace + runtime manifest (pinned to polkadot-sdk stable2409 + matching Frontier branch)
- [x] Runtime: System, Timestamp, Aura, GRANDPA, Balances (KIN, 18 dec), TxPayment, Sudo,
      Ethereum, EVM, EVMChainId, BaseFee (EIP-1559 style)
- [x] Dev genesis preset (Hardhat account #0 funded with 1,000,000 KIN)
- [~] runtime/src/apis.rs written; EthereumRuntimeRPCApi call/create methods still to copy from the Frontier template
- [~] node/ written (CLI, chain spec, service, RPC); needs compile pass against pinned Frontier release

## Placeholders to confirm
- CHAIN_ID = 7777 (runtime/src/lib.rs). Check chainlist.org for collisions before testnet.
- Gas limit 15M per block, base fee starts at 1 gwei-KIN.

## Run (after it compiles)
    cargo build --release
    ./target/release/voidkin-node --dev --rpc-cors all --rpc-external
    # MetaMask: RPC http://127.0.0.1:9944, chain id 7777, symbol KIN

## Build (needs network + Rust nightly/wasm target)
    rustup target add wasm32-unknown-unknown
    cargo build -p voidkin-runtime
