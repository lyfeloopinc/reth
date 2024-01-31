use reth_primitives::{revm::env::fill_block_env, Address, ChainSpec, Header, Transaction, U256};
use revm::{
    precompile::Precompiles,
    primitives::{BlockEnv, CfgEnv, SpecId, TxEnv},
};

/// This represents the set of methods used to configure the EVM before execution.
pub trait EvmEnvConfig: Send + Sync + Unpin + Clone {
    /// The type of the transaction metadata.
    type TxMeta;

    /// Fill transaction environment from a [Transaction] and the given sender address.
    fn fill_tx_env<T>(tx_env: &mut TxEnv, transaction: T, sender: Address, meta: Self::TxMeta)
    where
        T: AsRef<Transaction>;

    /// Fill [CfgEnv] fields according to the chain spec and given header
    fn fill_cfg_env(
        cfg_env: &mut CfgEnv,
        chain_spec: &ChainSpec,
        header: &Header,
        total_difficulty: U256,
    );

    /// Convenience function to call both [fill_cfg_env](EvmEnvConfig::fill_cfg_env) and
    /// [fill_block_env].
    fn fill_cfg_and_block_env(
        cfg: &mut CfgEnv,
        block_env: &mut BlockEnv,
        chain_spec: &ChainSpec,
        header: &Header,
        total_difficulty: U256,
    ) {
        Self::fill_cfg_env(cfg, chain_spec, header, total_difficulty);
        let after_merge = cfg.spec_id >= SpecId::MERGE;
        fill_block_env(block_env, chain_spec, header, after_merge);
    }
}

/// TODO: for further note: the custom precompiles will be configured in the builder, specifically
/// in the pre execution hoook.
///
/// https://github.com/bluealloy/revm/blob/ecb6c4b65461fcbcad6b51cf7ae9065181bb7617/crates/revm/src/handler/handle_types/pre_execution.rs#L25-L26
///
/// We also need `with_state` equivalent
trait EvmConfig: EvmEnvConfig {
    /// The type of executor used to execute transactions.
    ///
    /// This is at least an [ExecutorFactory] since the reth execution stage requires attaching a
    /// [StateProvider] to an executor.
    type Executor;

    /// Returns the precompiles used in execution.
    fn precompiles(&self) -> Precompiles;

    /// Returns the executor that will be used to execute transactions.
    fn executor(&self) -> Self::Executor;

    // this is also where all the handlers will be added with EvmBuilder
}
