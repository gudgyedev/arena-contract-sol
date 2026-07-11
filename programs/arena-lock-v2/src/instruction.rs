use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

use crate::state::{CONFIG_SEED, POSITION_SEED, VAULT_AUTHORITY_SEED};

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub enum ArenaInstruction {
    InitializeConfig {
        config_id: u64,
        min_lock_seconds: i64,
        activation_delay_seconds: i64,
        epoch_seconds: i64,
        min_deposit_amount: u64,
        early_exit_penalty_bps: u16,
        burn_penalty_bps: u16,
        decimals: u8,
    },
    Deposit {
        amount: u64,
    },
    ActivatePosition,
    FundRewards {
        amount: u64,
    },
    RollEpoch,
    FinalizeRewards,
    ClaimRewards,
    Withdraw {
        amount: u64,
    },
    FundRewardsChecked {
        amount: u64,
        expected_eligible_locked: u64,
        max_current_epoch: u64,
    },
}

pub fn config_pda(program_id: &Pubkey, authority: &Pubkey, config_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[CONFIG_SEED, authority.as_ref(), &config_id.to_le_bytes()],
        program_id,
    )
}

pub fn position_pda(program_id: &Pubkey, config: &Pubkey, owner: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[POSITION_SEED, config.as_ref(), owner.as_ref()],
        program_id,
    )
}

pub fn vault_authority_pda(program_id: &Pubkey, config: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[VAULT_AUTHORITY_SEED, config.as_ref()], program_id)
}

#[allow(clippy::too_many_arguments)]
pub fn initialize_config(
    program_id: Pubkey,
    authority: Pubkey,
    config_id: u64,
    min_lock_seconds: i64,
    activation_delay_seconds: i64,
    epoch_seconds: i64,
    min_deposit_amount: u64,
    early_exit_penalty_bps: u16,
    burn_penalty_bps: u16,
    mint: Pubkey,
    vault_token_account: Pubkey,
    reward_pool_token_account: Pubkey,
    token_program: Pubkey,
    decimals: u8,
) -> Instruction {
    let (config, _) = config_pda(&program_id, &authority, config_id);
    let (vault_authority, _) = vault_authority_pda(&program_id, &config);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(authority, true),
            AccountMeta::new(config, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(vault_token_account, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new_readonly(reward_pool_token_account, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: borsh::to_vec(&ArenaInstruction::InitializeConfig {
            config_id,
            min_lock_seconds,
            activation_delay_seconds,
            epoch_seconds,
            min_deposit_amount,
            early_exit_penalty_bps,
            burn_penalty_bps,
            decimals,
        })
        .expect("serialize initialize config"),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn deposit(
    program_id: Pubkey,
    authority: Pubkey,
    config_id: u64,
    owner: Pubkey,
    owner_token_account: Pubkey,
    vault_token_account: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
    amount: u64,
) -> Instruction {
    let (config, _) = config_pda(&program_id, &authority, config_id);
    let (position, _) = position_pda(&program_id, &config, &owner);
    let (vault_authority, _) = vault_authority_pda(&program_id, &config);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(owner_token_account, false),
            AccountMeta::new(config, false),
            AccountMeta::new(position, false),
            AccountMeta::new(vault_token_account, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: borsh::to_vec(&ArenaInstruction::Deposit { amount }).expect("serialize deposit"),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn activate_position(
    program_id: Pubkey,
    authority: Pubkey,
    config_id: u64,
    owner: Pubkey,
    reward_pool_token_account: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
) -> Instruction {
    let (config, _) = config_pda(&program_id, &authority, config_id);
    let (position, _) = position_pda(&program_id, &config, &owner);
    let (vault_authority, _) = vault_authority_pda(&program_id, &config);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(config, false),
            AccountMeta::new(position, false),
            AccountMeta::new(reward_pool_token_account, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new(mint, false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: borsh::to_vec(&ArenaInstruction::ActivatePosition)
            .expect("serialize activate position"),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn fund_rewards(
    program_id: Pubkey,
    authority: Pubkey,
    config_id: u64,
    funder: Pubkey,
    funder_token_account: Pubkey,
    reward_pool_token_account: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
    amount: u64,
) -> Instruction {
    let (config, _) = config_pda(&program_id, &authority, config_id);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(funder, true),
            AccountMeta::new(funder_token_account, false),
            AccountMeta::new(config, false),
            AccountMeta::new(reward_pool_token_account, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: borsh::to_vec(&ArenaInstruction::FundRewards { amount })
            .expect("serialize fund rewards"),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn fund_rewards_checked(
    program_id: Pubkey,
    funder: Pubkey,
    config_id: u64,
    authority: Pubkey,
    funder_token_account: Pubkey,
    reward_pool_token_account: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
    amount: u64,
    expected_eligible_locked: u64,
    max_current_epoch: u64,
) -> Instruction {
    let (config, _) = config_pda(&program_id, &authority, config_id);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(funder, true),
            AccountMeta::new(funder_token_account, false),
            AccountMeta::new(config, false),
            AccountMeta::new(reward_pool_token_account, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: borsh::to_vec(&ArenaInstruction::FundRewardsChecked {
            amount,
            expected_eligible_locked,
            max_current_epoch,
        })
        .expect("serialize checked fund rewards"),
    }
}

pub fn roll_epoch(
    program_id: Pubkey,
    authority: Pubkey,
    config_id: u64,
    reward_pool_token_account: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
) -> Instruction {
    let (config, _) = config_pda(&program_id, &authority, config_id);
    let (vault_authority, _) = vault_authority_pda(&program_id, &config);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(config, false),
            AccountMeta::new(reward_pool_token_account, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new(mint, false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: borsh::to_vec(&ArenaInstruction::RollEpoch).expect("serialize roll epoch"),
    }
}

pub fn finalize_rewards(
    program_id: Pubkey,
    authority: Pubkey,
    config_id: u64,
    reward_pool_token_account: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
) -> Instruction {
    let (config, _) = config_pda(&program_id, &authority, config_id);
    let (vault_authority, _) = vault_authority_pda(&program_id, &config);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(config, false),
            AccountMeta::new(reward_pool_token_account, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new(mint, false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: borsh::to_vec(&ArenaInstruction::FinalizeRewards)
            .expect("serialize finalize rewards"),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn claim_rewards(
    program_id: Pubkey,
    authority: Pubkey,
    config_id: u64,
    owner: Pubkey,
    owner_token_account: Pubkey,
    reward_pool_token_account: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
) -> Instruction {
    let (config, _) = config_pda(&program_id, &authority, config_id);
    let (position, _) = position_pda(&program_id, &config, &owner);
    let (vault_authority, _) = vault_authority_pda(&program_id, &config);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(owner_token_account, false),
            AccountMeta::new(config, false),
            AccountMeta::new(position, false),
            AccountMeta::new(reward_pool_token_account, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: borsh::to_vec(&ArenaInstruction::ClaimRewards).expect("serialize claim rewards"),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn withdraw(
    program_id: Pubkey,
    authority: Pubkey,
    config_id: u64,
    owner: Pubkey,
    owner_token_account: Pubkey,
    vault_token_account: Pubkey,
    reward_pool_token_account: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
    amount: u64,
) -> Instruction {
    let (config, _) = config_pda(&program_id, &authority, config_id);
    let (position, _) = position_pda(&program_id, &config, &owner);
    let (vault_authority, _) = vault_authority_pda(&program_id, &config);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(owner_token_account, false),
            AccountMeta::new(config, false),
            AccountMeta::new(position, false),
            AccountMeta::new(vault_token_account, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new(reward_pool_token_account, false),
            AccountMeta::new(mint, false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: borsh::to_vec(&ArenaInstruction::Withdraw { amount }).expect("serialize withdraw"),
    }
}
