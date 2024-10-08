//! Block abstraction.

pub mod body;

use alloc::{fmt, vec::Vec};

use alloy_consensus::BlockHeader;
use alloy_primitives::{Address, Sealable, B256};

use crate::{traits::BlockBody, BlockWithSenders, SealedBlock};

/// Helper trait, unifies behaviour required of a block header.
pub trait Header: BlockHeader + Sealable {}

impl<T> Header for T where T: BlockHeader + Sealable {}

/// Abstraction of block data type.
// todo: make sealable super-trait, depends on <https://github.com/paradigmxyz/reth/issues/11449>
// todo: make with senders extension trait, so block can be impl by block type already containing
// senders
pub trait Block:
    fmt::Debug
    + Clone
    + PartialEq
    + Eq
    + Default
    + serde::Serialize
    + for<'a> serde::Deserialize<'a>
    + From<(Self::Header, Self::Body)>
    + Into<(Self::Header, Self::Body)>
{
    /// Header part of the block.
    type Header: Header;

    /// The block's body contains the transactions in the block.
    type Body: BlockBody;

    /// Returns reference to [`BlockHeader`] type.
    fn header(&self) -> &Self::Header;

    /// Returns reference to [`BlockBody`] type.
    fn body(&self) -> &Self::Body;

    /// Calculate the header hash and seal the block so that it can't be changed.
    fn seal_slow(self) -> SealedBlock;
    /// Seal the block with a known hash.
    ///
    /// WARNING: This method does not perform validation whether the hash is correct.
    fn seal(self, hash: B256) -> SealedBlock;

    /// Expensive operation that recovers transaction signer. See
    /// [`SealedBlockWithSenders`](crate::SealedBlockWithSenders).
    fn senders(&self) -> Option<Vec<Address>> {
        self.body().recover_signers()
    }

    /// Transform into a [`BlockWithSenders`].
    ///
    /// # Panics
    ///
    /// If the number of senders does not match the number of transactions in the block
    /// and the signer recovery for one of the transactions fails.
    ///
    /// Note: this is expected to be called with blocks read from disk.
    #[track_caller]
    fn with_senders_unchecked(self, senders: Vec<Address>) -> BlockWithSenders {
        self.try_with_senders_unchecked(senders).expect("stored block is valid")
    }

    /// Transform into a [`BlockWithSenders`] using the given senders.
    ///
    /// If the number of senders does not match the number of transactions in the block, this falls
    /// back to manually recovery, but _without ensuring that the signature has a low `s` value_.
    /// See also [`TransactionSigned::recover_signer_unchecked`](crate::TransactionSigned).
    ///
    /// Returns an error if a signature is invalid.
    #[track_caller]
    fn try_with_senders_unchecked(self, senders: Vec<Address>) -> Result<BlockWithSenders, Self>;

    /// **Expensive**. Transform into a [`BlockWithSenders`] by recovering senders in the contained
    /// transactions.
    ///
    /// Returns `None` if a transaction is invalid.
    fn with_recovered_senders(self) -> Option<BlockWithSenders>;

    /// Calculates a heuristic for the in-memory size of the [`Block`].
    fn size(&self) -> usize;
}
