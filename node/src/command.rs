use crate::{chain_spec, cli::{Cli, Subcommand}, service};
use sc_cli::SubstrateCli;
use sc_service::PartialComponents;

impl SubstrateCli for Cli {
    fn impl_name() -> String { "VOIDKIN Node".into() }
    fn impl_version() -> String { env!("SUBSTRATE_CLI_IMPL_VERSION").into() }
    fn description() -> String { "VOIDKIN: EVM-compatible Layer 1 (native token KIN)".into() }
    fn author() -> String { "VOIDKIN".into() }
    fn support_url() -> String { "https://github.com/voidkin/voidkin/issues".into() }
    fn copyright_start_year() -> i32 { 2026 }

    fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
        Ok(match id {
            "dev" => Box::new(chain_spec::development_config()?),
            "local" => Box::new(chain_spec::local_testnet_config()?),
            path => Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(path))?),
        })
    }
}

pub fn run() -> sc_cli::Result<()> {
    let cli = Cli::from_args();
    match &cli.subcommand {
        Some(Subcommand::Key(cmd)) => cmd.run(&cli),
        Some(Subcommand::BuildSpec(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
        }
        Some(Subcommand::CheckBlock(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents { client, task_manager, import_queue, .. } = service::new_partial(&config, &cli.eth)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::ExportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents { client, task_manager, .. } = service::new_partial(&config, &cli.eth)?;
                Ok((cmd.run(client, config.database), task_manager))
            })
        }
        Some(Subcommand::ImportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents { client, task_manager, import_queue, .. } = service::new_partial(&config, &cli.eth)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::PurgeChain(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run(config.database))
        }
        Some(Subcommand::Revert(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents { client, task_manager, backend, .. } = service::new_partial(&config, &cli.eth)?;
                let aux_revert = Box::new(|client, _, blocks| {
                    sc_consensus_grandpa::revert(client, blocks)?;
                    Ok(())
                });
                Ok((cmd.run(client, backend, Some(aux_revert)), task_manager))
            })
        }
        None => {
            let runner = cli.create_runner(&cli.run)?;
            runner.run_node_until_exit(|config| async move {
                service::new_full(config, cli.eth).map_err(sc_cli::Error::Service)
            })
        }
    }
}
