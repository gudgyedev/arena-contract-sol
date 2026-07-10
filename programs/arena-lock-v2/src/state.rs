use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{program_error::ProgramError, pubkey::Pubkey};

use crate::error::ArenaError;

pub const CONFIG_SEED: &[u8] = b"arena-config";
pub const POSITION_SEED: &[u8] = b"arena-position";
pub const VAULT_AUTHORITY_SEED: &[u8] = b"arena-vault-authority";
pub const CONFIG_SIZE: usize = 640;
pub const POSITION_SIZE: usize = 384;
pub const MAX_ARENA_EARLY_EXIT_PENALTY_BPS: u16 = 5_000;
pub const REWARD_INDEX_SCALE: u128 = 1_000_000_000_000_000_000;

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct ArenaConfig {
    pub is_initialized: bool,
    pub config_id: u64,
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub token_program: Pubkey,
    pub vault_token_account: Pubkey,
    pub reward_pool_token_account: Pubkey,
    pub min_lock_seconds: i64,
    pub activation_delay_seconds: i64,
    pub epoch_seconds: i64,
    pub min_deposit_amount: u64,
    pub early_exit_penalty_bps: u16,
    pub burn_penalty_bps: u16,
    pub decimals: u8,
    pub config_bump: u8,
    pub vault_authority_bump: u8,
    pub created_ts: i64,
    pub last_epoch_ts: i64,
    pub current_epoch: u64,
    pub total_locked: u64,
    pub eligible_locked: u64,
    pub pending_activation_locked: u64,
    pub pending_rewards: u64,
    pub total_rewards_funded: u64,
    pub total_rewards_distributed: u64,
    pub total_rewards_claimed: u64,
    pub total_penalties_collected: u64,
    pub total_burned: u64,
    pub reward_index: u128,
    pub total_rewards_expired: u64,
}

impl ArenaConfig {
    pub fn load(data: &[u8]) -> Result<Self, ProgramError> {
        let mut slice = data;
        let config = Self::deserialize(&mut slice).map_err(|_| ArenaError::NotInitialized)?;
        if !config.is_initialized {
            return Err(ArenaError::NotInitialized.into());
        }
        Ok(config)
    }

    pub fn store(&self, data: &mut [u8]) -> Result<(), ProgramError> {
        let encoded = borsh::to_vec(self).map_err(|_| ArenaError::InvalidInstruction)?;
        if encoded.len() > data.len() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        data.fill(0);
        data[..encoded.len()].copy_from_slice(&encoded);
        Ok(())
    }
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct ArenaPosition {
    pub is_initialized: bool,
    pub config: Pubkey,
    pub owner: Pubkey,
    pub locked_amount: u64,
    pub eligible_amount: u64,
    pub pending_activation_amount: u64,
    pub total_deposited: u64,
    pub total_withdrawn: u64,
    pub total_penalty_paid: u64,
    pub total_burned: u64,
    pub total_rewards_claimed: u64,
    pub pending_rewards: u64,
    pub lock_start_ts: i64,
    pub unlock_ts: i64,
    pub activation_ts: i64,
    pub last_activity_ts: i64,
    pub reward_index_checkpoint: u128,
    pub position_bump: u8,
}

impl ArenaPosition {
    pub fn load(data: &[u8]) -> Result<Self, ProgramError> {
        let mut slice = data;
        let position = Self::deserialize(&mut slice).map_err(|_| ArenaError::NotInitialized)?;
        if !position.is_initialized {
            return Err(ArenaError::NotInitialized.into());
        }
        Ok(position)
    }

    pub fn store(&self, data: &mut [u8]) -> Result<(), ProgramError> {
        let encoded = borsh::to_vec(self).map_err(|_| ArenaError::InvalidInstruction)?;
        if encoded.len() > data.len() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        data.fill(0);
        data[..encoded.len()].copy_from_slice(&encoded);
        Ok(())
    }
}
