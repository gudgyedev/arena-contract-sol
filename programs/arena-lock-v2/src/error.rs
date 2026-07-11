use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ArenaError {
    #[error("Invalid instruction data")]
    InvalidInstruction = 11000,
    #[error("Invalid PDA seeds")]
    InvalidPda,
    #[error("Account is already initialized")]
    AlreadyInitialized,
    #[error("Account is not initialized")]
    NotInitialized,
    #[error("Invalid account owner")]
    InvalidOwner,
    #[error("Invalid signer")]
    InvalidSigner,
    #[error("Invalid amount")]
    InvalidAmount,
    #[error("Invalid penalty")]
    InvalidPenalty,
    #[error("Invalid burn")]
    InvalidBurn,
    #[error("Epoch is not ready")]
    EpochNotReady,
    #[error("Position is not ready for activation")]
    ActivationNotReady,
    #[error("No rewards are claimable")]
    NoRewards,
    #[error("No eligible stake")]
    NoEligibleStake,
    #[error("Insufficient position balance")]
    InsufficientPositionBalance,
    #[error("Invalid treasury account")]
    InvalidTreasury,
    #[error("Invalid token program")]
    InvalidTokenProgram,
    #[error("Invalid token mint")]
    InvalidTokenMint,
    #[error("Invalid token account")]
    InvalidTokenAccount,
    #[error("Math overflow")]
    MathOverflow,
    #[error("Account is not writable")]
    AccountNotWritable,
    #[error("Unsupported state version")]
    UnsupportedStateVersion,
    #[error("Funding snapshot mismatch")]
    FundingSnapshotMismatch,
}

impl From<ArenaError> for ProgramError {
    fn from(error: ArenaError) -> Self {
        ProgramError::Custom(error as u32)
    }
}
