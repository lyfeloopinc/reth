//! Loads and formats OP transaction RPC response.

use alloc::{fmt, sync::Arc};

use alloy_primitives::{Bytes, B256};
use alloy_rpc_types::TransactionInfo;
use derive_more::Constructor;
use op_alloy_rpc_types::Transaction;
use reth_node_api::FullNodeComponents;
use reth_primitives::TransactionSignedEcRecovered;
use reth_provider::{BlockReaderIdExt, TransactionsProvider};
use reth_rpc::eth::EthTxBuilder;
use reth_rpc_eth_api::{
    helpers::{EthSigner, EthTransactions, LoadTransaction, SpawnBlocking},
    FromEthApiError, FullEthApiTypes, TransactionCompat,
};
use reth_rpc_eth_types::{utils::recover_raw_transaction, EthStateCache};
use reth_transaction_pool::{PoolTransaction, TransactionOrigin, TransactionPool};

use crate::{OpEthApi, SequencerClient};

impl<N> EthTransactions for OpEthApi<N>
where
    Self: LoadTransaction,
    N: FullNodeComponents,
{
    fn provider(&self) -> impl BlockReaderIdExt {
        self.inner.provider()
    }

    fn signers(&self) -> &parking_lot::RwLock<Vec<Box<dyn EthSigner>>> {
        self.inner.signers()
    }

    /// Decodes and recovers the transaction and submits it to the pool.
    ///
    /// Returns the hash of the transaction.
    async fn send_raw_transaction(&self, tx: Bytes) -> Result<B256, Self::Error> {
        let recovered = recover_raw_transaction(tx.clone())?;
        let pool_transaction =
            <Self::Pool as TransactionPool>::Transaction::from_pooled(recovered.into());

        // On optimism, transactions are forwarded directly to the sequencer to be included in
        // blocks that it builds.
        if let Some(client) = self.raw_tx_forwarder().as_ref() {
            tracing::debug!( target: "rpc::eth",  "forwarding raw transaction to");
            let _ = client.forward_raw_transaction(&tx).await.inspect_err(|err| {
                    tracing::debug!(target: "rpc::eth", %err, hash=% *pool_transaction.hash(), "failed to forward raw transaction");
                });
        }

        // submit the transaction to the pool with a `Local` origin
        let hash = self
            .pool()
            .add_transaction(TransactionOrigin::Local, pool_transaction)
            .await
            .map_err(Self::Error::from_eth_err)?;

        Ok(hash)
    }
}

impl<N> LoadTransaction for OpEthApi<N>
where
    Self: SpawnBlocking + FullEthApiTypes,
    N: FullNodeComponents,
{
    type Pool = N::Pool;

    fn provider(&self) -> impl TransactionsProvider {
        self.inner.provider()
    }

    fn cache(&self) -> &EthStateCache {
        self.inner.cache()
    }

    fn pool(&self) -> &Self::Pool {
        self.inner.pool()
    }
}

impl<N> OpEthApi<N>
where
    N: FullNodeComponents,
{
    /// Sets a [`SequencerClient`] for `eth_sendRawTransaction` to forward transactions to.
    pub fn set_sequencer_client(
        &self,
        sequencer_client: SequencerClient,
    ) -> Result<(), tokio::sync::SetError<SequencerClient>> {
        self.sequencer_client.set(sequencer_client)
    }

    /// Returns the [`SequencerClient`] if one is set.
    pub fn raw_tx_forwarder(&self) -> Option<SequencerClient> {
        self.sequencer_client.get().cloned()
    }
}

/// Builds OP transaction response type.
#[derive(Debug, Constructor)]
pub struct OpTxBuilder<T> {
    /// Chain spec.
    pub chain_spec: Arc<T>,
}

impl<T> TransactionCompat for OpTxBuilder<T>
where
    Self: Send + Sync,
    T: fmt::Debug,
{
    type Transaction = Transaction;

    fn fill(
        &self,
        tx: TransactionSignedEcRecovered,
        tx_info: TransactionInfo,
    ) -> Self::Transaction {
        let signed_tx = tx.clone().into_signed();

        let mut inner = EthTxBuilder.fill(tx, tx_info).inner;

        if signed_tx.is_deposit() {
            inner.gas_price = Some(signed_tx.max_fee_per_gas())
        }

        Transaction {
            inner,
            source_hash: signed_tx.source_hash(),
            mint: signed_tx.mint(),
            // only include is_system_tx if true: <https://github.com/ethereum-optimism/op-geth/blob/641e996a2dcf1f81bac9416cb6124f86a69f1de7/internal/ethapi/api.go#L1518-L1518>
            is_system_tx: (signed_tx.is_deposit() && signed_tx.is_system_transaction())
                .then_some(true),
            deposit_receipt_version: None, // todo: how to fill this field?
        }
    }

    fn otterscan_api_truncate_input(tx: &mut Self::Transaction) {
        tx.inner.input = tx.inner.input.slice(..4);
    }

    fn tx_type(tx: &Self::Transaction) -> u8 {
        tx.inner.transaction_type.unwrap_or_default()
    }
}

impl<T> Clone for OpTxBuilder<T> {
    fn clone(&self) -> Self {
        Self { chain_spec: self.chain_spec.clone() }
    }
}
