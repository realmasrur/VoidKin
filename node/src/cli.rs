#[derive(Debug, clap::Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Option<Subcommand>,
    #[clap(flatten)]
    pub run: sc_cli::RunCmd,
    #[command(flatten)]
    pub eth: EthConfiguration,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Key management (generate Aura/GRANDPA validator keys).
    #[command(subcommand)]
    Key(sc_cli::KeySubcommand),
    BuildSpec(sc_cli::BuildSpecCmd),
    CheckBlock(sc_cli::CheckBlockCmd),
    ExportBlocks(sc_cli::ExportBlocksCmd),
    ImportBlocks(sc_cli::ImportBlocksCmd),
    PurgeChain(sc_cli::PurgeChainCmd),
    Revert(sc_cli::RevertCmd),
}

/// Ethereum RPC options.
#[derive(Clone, Debug, clap::Args)]
pub struct EthConfiguration {
    /// Maximum number of logs returned by eth_getLogs.
    #[arg(long, default_value = "10000")]
    pub max_past_logs: u32,
    /// Maximum fee history cache size.
    #[arg(long, default_value = "2048")]
    pub fee_history_limit: u64,
    /// Size in MB of the Ethereum block/status cache.
    #[arg(long, default_value = "10")]
    pub eth_log_block_cache: usize,
    #[arg(long, default_value = "10")]
    pub eth_statuses_cache: usize,
}
