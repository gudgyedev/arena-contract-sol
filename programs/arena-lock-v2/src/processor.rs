use borsh::BorshDeserialize;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction, system_program,
    sysvar::Sysvar,
};
use spl_token_2022::extension::{BaseStateWithExtensions, StateWithExtensions};

use crate::{
    error::ArenaError,
    instruction::ArenaInstruction,
    state::{
        ArenaConfig, ArenaPosition, CONFIG_SEED, CONFIG_SIZE, MAX_ARENA_EARLY_EXIT_PENALTY_BPS,
        POSITION_SEED, POSITION_SIZE, REWARD_INDEX_SCALE, VAULT_AUTHORITY_SEED,
    },
};

const BPS_DENOMINATOR: u64 = 10_000;

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = ArenaInstruction::try_from_slice(instruction_data)
        .map_err(|_| ArenaError::InvalidInstruction)?;

    match instruction {
        ArenaInstruction::InitializeConfig {
            config_id,
            min_lock_seconds,
            activation_delay_seconds,
            epoch_seconds,
            min_deposit_amount,
            early_exit_penalty_bps,
            burn_penalty_bps,
            decimals,
        } => process_initialize_config(
            program_id,
            accounts,
            config_id,
            min_lock_seconds,
            activation_delay_seconds,
            epoch_seconds,
            min_deposit_amount,
            early_exit_penalty_bps,
            burn_penalty_bps,
            decimals,
        ),
        ArenaInstruction::Deposit { amount } => process_deposit(program_id, accounts, amount),
        ArenaInstruction::ActivatePosition => process_activate_position(program_id, accounts),
        ArenaInstruction::FundRewards { amount } => {
            process_fund_rewards(program_id, accounts, amount)
        }
        ArenaInstruction::RollEpoch => process_roll_epoch(program_id, accounts),
        ArenaInstruction::ClaimRewards => process_claim_rewards(program_id, accounts),
        ArenaInstruction::Withdraw { amount } => process_withdraw(program_id, accounts, amount),
    }
}

fn assert_signer(account: &AccountInfo) -> ProgramResult {
    if !account.is_signer {
        return Err(ArenaError::InvalidSigner.into());
    }
    Ok(())
}

fn assert_owned_by(account: &AccountInfo, owner: &Pubkey) -> ProgramResult {
    if account.owner != owner {
        return Err(ArenaError::InvalidOwner.into());
    }
    Ok(())
}

fn assert_system_program(account: &AccountInfo) -> ProgramResult {
    if *account.key != system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

fn assert_supported_token_program(account: &AccountInfo) -> ProgramResult {
    if *account.key != spl_token::id() && *account.key != spl_token_2022::id() {
        return Err(ArenaError::InvalidTokenProgram.into());
    }
    Ok(())
}

fn assert_uninitialized(account: &AccountInfo) -> ProgramResult {
    if !account.data_is_empty() || account.owner != &system_program::id() {
        return Err(ArenaError::AlreadyInitialized.into());
    }
    Ok(())
}

fn create_program_account<'a>(
    payer: &AccountInfo<'a>,
    new_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    program_id: &Pubkey,
    space: usize,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let rent = Rent::get()?;
    let required_lamports = rent.minimum_balance(space);
    let existing_lamports = new_account.lamports();
    if existing_lamports < required_lamports {
        let top_up = required_lamports
            .checked_sub(existing_lamports)
            .ok_or(ArenaError::MathOverflow)?;
        invoke(
            &system_instruction::transfer(payer.key, new_account.key, top_up),
            &[payer.clone(), new_account.clone(), system_program.clone()],
        )?;
    }
    invoke_signed(
        &system_instruction::allocate(new_account.key, space as u64),
        &[new_account.clone(), system_program.clone()],
        &[signer_seeds],
    )?;
    invoke_signed(
        &system_instruction::assign(new_account.key, program_id),
        &[new_account.clone(), system_program.clone()],
        &[signer_seeds],
    )
}

fn transfer_checked_ix(
    token_program: &Pubkey,
    source: &Pubkey,
    mint: &Pubkey,
    destination: &Pubkey,
    authority: &Pubkey,
    amount: u64,
    decimals: u8,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    if *token_program == spl_token::id() {
        spl_token::instruction::transfer_checked(
            token_program,
            source,
            mint,
            destination,
            authority,
            &[],
            amount,
            decimals,
        )
    } else if *token_program == spl_token_2022::id() {
        spl_token_2022::instruction::transfer_checked(
            token_program,
            source,
            mint,
            destination,
            authority,
            &[],
            amount,
            decimals,
        )
    } else {
        Err(ArenaError::InvalidTokenProgram.into())
    }
}

fn burn_checked_ix(
    token_program: &Pubkey,
    source: &Pubkey,
    mint: &Pubkey,
    authority: &Pubkey,
    amount: u64,
    decimals: u8,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    if *token_program == spl_token::id() {
        spl_token::instruction::burn_checked(
            token_program,
            source,
            mint,
            authority,
            &[],
            amount,
            decimals,
        )
    } else if *token_program == spl_token_2022::id() {
        spl_token_2022::instruction::burn_checked(
            token_program,
            source,
            mint,
            authority,
            &[],
            amount,
            decimals,
        )
    } else {
        Err(ArenaError::InvalidTokenProgram.into())
    }
}

#[derive(Clone, Copy)]
struct TokenAccountView {
    mint: Pubkey,
    owner: Pubkey,
}

fn unpack_token_account(
    token_program: &Pubkey,
    account: &AccountInfo,
) -> Result<TokenAccountView, ProgramError> {
    if account.owner != token_program {
        return Err(ArenaError::InvalidTokenAccount.into());
    }
    if *token_program == spl_token::id() {
        let token_account = spl_token::state::Account::unpack(&account.data.borrow())
            .map_err(|_| ArenaError::InvalidTokenAccount)?;
        Ok(TokenAccountView {
            mint: token_account.mint,
            owner: token_account.owner,
        })
    } else if *token_program == spl_token_2022::id() {
        let account_data = account.data.borrow();
        let token_account =
            StateWithExtensions::<spl_token_2022::state::Account>::unpack(&account_data)
                .map_err(|_| ArenaError::InvalidTokenAccount)?;
        if !token_account
            .get_extension_types()
            .map_err(|_| ArenaError::InvalidTokenAccount)?
            .is_empty()
        {
            return Err(ArenaError::InvalidTokenAccount.into());
        }
        Ok(TokenAccountView {
            mint: token_account.base.mint,
            owner: token_account.base.owner,
        })
    } else {
        Err(ArenaError::InvalidTokenProgram.into())
    }
}

fn mint_decimals(mint: &AccountInfo, token_program: &Pubkey) -> Result<u8, ProgramError> {
    if mint.owner != token_program {
        return Err(ArenaError::InvalidTokenMint.into());
    }
    if *token_program == spl_token::id() {
        Ok(spl_token::state::Mint::unpack(&mint.data.borrow())
            .map_err(|_| ArenaError::InvalidTokenMint)?
            .decimals)
    } else if *token_program == spl_token_2022::id() {
        let mint_data = mint.data.borrow();
        let mint_state = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)
            .map_err(|_| ArenaError::InvalidTokenMint)?;
        if !mint_state
            .get_extension_types()
            .map_err(|_| ArenaError::InvalidTokenMint)?
            .is_empty()
        {
            return Err(ArenaError::InvalidTokenMint.into());
        }
        Ok(mint_state.base.decimals)
    } else {
        Err(ArenaError::InvalidTokenProgram.into())
    }
}

fn assert_token_account(
    token_program: &AccountInfo,
    account: &AccountInfo,
    mint: &Pubkey,
    owner: Option<&Pubkey>,
) -> ProgramResult {
    let view = unpack_token_account(token_program.key, account)?;
    if view.mint != *mint {
        return Err(ArenaError::InvalidTokenMint.into());
    }
    if owner.is_some_and(|expected| view.owner != *expected) {
        return Err(ArenaError::InvalidTokenAccount.into());
    }
    Ok(())
}

fn accrue_rewards(config: &ArenaConfig, position: &mut ArenaPosition) -> ProgramResult {
    if config.reward_index < position.reward_index_checkpoint {
        return Err(ArenaError::MathOverflow.into());
    }
    let delta = config
        .reward_index
        .checked_sub(position.reward_index_checkpoint)
        .ok_or(ArenaError::MathOverflow)?;
    if delta > 0 && position.eligible_amount > 0 {
        let earned = u128::from(position.eligible_amount)
            .checked_mul(delta)
            .ok_or(ArenaError::MathOverflow)?
            .checked_div(REWARD_INDEX_SCALE)
            .ok_or(ArenaError::MathOverflow)?;
        let earned = u64::try_from(earned).map_err(|_| ArenaError::MathOverflow)?;
        position.pending_rewards = position
            .pending_rewards
            .checked_add(earned)
            .ok_or(ArenaError::MathOverflow)?;
    }
    position.reward_index_checkpoint = config.reward_index;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_initialize_config(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    config_id: u64,
    min_lock_seconds: i64,
    activation_delay_seconds: i64,
    epoch_seconds: i64,
    min_deposit_amount: u64,
    early_exit_penalty_bps: u16,
    burn_penalty_bps: u16,
    decimals: u8,
) -> ProgramResult {
    if min_lock_seconds <= 0
        || activation_delay_seconds <= 0
        || epoch_seconds <= 0
        || min_deposit_amount == 0
    {
        return Err(ArenaError::InvalidAmount.into());
    }
    if early_exit_penalty_bps > MAX_ARENA_EARLY_EXIT_PENALTY_BPS {
        return Err(ArenaError::InvalidPenalty.into());
    }
    if burn_penalty_bps > early_exit_penalty_bps {
        return Err(ArenaError::InvalidBurn.into());
    }

    let account_info_iter = &mut accounts.iter();
    let authority = next_account_info(account_info_iter)?;
    let config = next_account_info(account_info_iter)?;
    let mint = next_account_info(account_info_iter)?;
    let vault_token_account = next_account_info(account_info_iter)?;
    let vault_authority = next_account_info(account_info_iter)?;
    let reward_pool_token_account = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;

    assert_signer(authority)?;
    assert_uninitialized(config)?;
    assert_supported_token_program(token_program)?;
    assert_system_program(system_program)?;
    if mint_decimals(mint, token_program.key)? != decimals {
        return Err(ArenaError::InvalidTokenMint.into());
    }

    let (expected_config, config_bump) = Pubkey::find_program_address(
        &[
            CONFIG_SEED,
            authority.key.as_ref(),
            &config_id.to_le_bytes(),
        ],
        program_id,
    );
    if expected_config != *config.key {
        return Err(ArenaError::InvalidPda.into());
    }
    let (expected_vault_authority, vault_authority_bump) =
        Pubkey::find_program_address(&[VAULT_AUTHORITY_SEED, config.key.as_ref()], program_id);
    if expected_vault_authority != *vault_authority.key {
        return Err(ArenaError::InvalidPda.into());
    }
    if vault_token_account.key == reward_pool_token_account.key {
        return Err(ArenaError::InvalidTreasury.into());
    }
    assert_token_account(
        token_program,
        vault_token_account,
        mint.key,
        Some(vault_authority.key),
    )?;
    assert_token_account(
        token_program,
        reward_pool_token_account,
        mint.key,
        Some(vault_authority.key),
    )?;

    create_program_account(
        authority,
        config,
        system_program,
        program_id,
        CONFIG_SIZE,
        &[
            CONFIG_SEED,
            authority.key.as_ref(),
            &config_id.to_le_bytes(),
            &[config_bump],
        ],
    )?;

    let now = Clock::get()?.unix_timestamp;
    let arena_config = ArenaConfig {
        is_initialized: true,
        config_id,
        authority: *authority.key,
        mint: *mint.key,
        token_program: *token_program.key,
        vault_token_account: *vault_token_account.key,
        reward_pool_token_account: *reward_pool_token_account.key,
        min_lock_seconds,
        activation_delay_seconds,
        epoch_seconds,
        min_deposit_amount,
        early_exit_penalty_bps,
        burn_penalty_bps,
        decimals,
        config_bump,
        vault_authority_bump,
        created_ts: now,
        last_epoch_ts: now,
        current_epoch: 0,
        total_locked: 0,
        eligible_locked: 0,
        pending_activation_locked: 0,
        pending_rewards: 0,
        total_rewards_funded: 0,
        total_rewards_distributed: 0,
        total_rewards_claimed: 0,
        total_penalties_collected: 0,
        total_burned: 0,
        reward_index: 0,
    };
    arena_config.store(&mut config.data.borrow_mut())
}

fn process_deposit(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let owner = next_account_info(account_info_iter)?;
    let owner_token_account = next_account_info(account_info_iter)?;
    let config = next_account_info(account_info_iter)?;
    let position = next_account_info(account_info_iter)?;
    let vault_token_account = next_account_info(account_info_iter)?;
    let vault_authority = next_account_info(account_info_iter)?;
    let mint = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;

    assert_signer(owner)?;
    assert_owned_by(config, program_id)?;
    assert_supported_token_program(token_program)?;
    assert_system_program(system_program)?;

    let mut arena_config = ArenaConfig::load(&config.data.borrow())?;
    if amount < arena_config.min_deposit_amount {
        return Err(ArenaError::InvalidAmount.into());
    }
    if arena_config.mint != *mint.key
        || arena_config.token_program != *token_program.key
        || arena_config.vault_token_account != *vault_token_account.key
    {
        return Err(ArenaError::InvalidTokenMint.into());
    }

    let (expected_vault_authority, _) =
        Pubkey::find_program_address(&[VAULT_AUTHORITY_SEED, config.key.as_ref()], program_id);
    if expected_vault_authority != *vault_authority.key {
        return Err(ArenaError::InvalidPda.into());
    }
    let (expected_position, position_bump) = Pubkey::find_program_address(
        &[POSITION_SEED, config.key.as_ref(), owner.key.as_ref()],
        program_id,
    );
    if expected_position != *position.key {
        return Err(ArenaError::InvalidPda.into());
    }

    assert_token_account(
        token_program,
        owner_token_account,
        mint.key,
        Some(owner.key),
    )?;
    assert_token_account(
        token_program,
        vault_token_account,
        mint.key,
        Some(vault_authority.key),
    )?;

    if position.data_is_empty() {
        create_program_account(
            owner,
            position,
            system_program,
            program_id,
            POSITION_SIZE,
            &[
                POSITION_SEED,
                config.key.as_ref(),
                owner.key.as_ref(),
                &[position_bump],
            ],
        )?;
    } else {
        assert_owned_by(position, program_id)?;
    }

    let ix = transfer_checked_ix(
        token_program.key,
        owner_token_account.key,
        mint.key,
        vault_token_account.key,
        owner.key,
        amount,
        arena_config.decimals,
    )?;
    invoke(
        &ix,
        &[
            owner_token_account.clone(),
            mint.clone(),
            vault_token_account.clone(),
            owner.clone(),
            token_program.clone(),
        ],
    )?;

    let now = Clock::get()?.unix_timestamp;
    let mut arena_position = if position.data.borrow().iter().any(|byte| *byte != 0) {
        let mut existing = ArenaPosition::load(&position.data.borrow())?;
        if existing.config != *config.key || existing.owner != *owner.key {
            return Err(ArenaError::InvalidPda.into());
        }
        accrue_rewards(&arena_config, &mut existing)?;
        existing
    } else {
        ArenaPosition {
            is_initialized: true,
            config: *config.key,
            owner: *owner.key,
            locked_amount: 0,
            eligible_amount: 0,
            pending_activation_amount: 0,
            total_deposited: 0,
            total_withdrawn: 0,
            total_penalty_paid: 0,
            total_burned: 0,
            total_rewards_claimed: 0,
            pending_rewards: 0,
            lock_start_ts: now,
            unlock_ts: 0,
            activation_ts: 0,
            last_activity_ts: 0,
            reward_index_checkpoint: arena_config.reward_index,
            position_bump,
        }
    };

    arena_position.locked_amount = arena_position
        .locked_amount
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_position.pending_activation_amount = arena_position
        .pending_activation_amount
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_position.total_deposited = arena_position
        .total_deposited
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_position.lock_start_ts = now;
    arena_position.unlock_ts = now
        .checked_add(arena_config.min_lock_seconds)
        .ok_or(ArenaError::MathOverflow)?;
    arena_position.activation_ts = now
        .checked_add(arena_config.activation_delay_seconds)
        .ok_or(ArenaError::MathOverflow)?;
    arena_position.last_activity_ts = now;

    arena_config.total_locked = arena_config
        .total_locked
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_config.pending_activation_locked = arena_config
        .pending_activation_locked
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;

    arena_config.store(&mut config.data.borrow_mut())?;
    arena_position.store(&mut position.data.borrow_mut())
}

fn process_activate_position(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let owner = next_account_info(account_info_iter)?;
    let config = next_account_info(account_info_iter)?;
    let position = next_account_info(account_info_iter)?;

    assert_signer(owner)?;
    assert_owned_by(config, program_id)?;
    assert_owned_by(position, program_id)?;

    let mut arena_config = ArenaConfig::load(&config.data.borrow())?;
    let mut arena_position = ArenaPosition::load(&position.data.borrow())?;
    if arena_position.config != *config.key || arena_position.owner != *owner.key {
        return Err(ArenaError::InvalidPda.into());
    }
    if arena_position.pending_activation_amount == 0 {
        return Err(ArenaError::InvalidAmount.into());
    }
    let now = Clock::get()?.unix_timestamp;
    if now < arena_position.activation_ts {
        return Err(ArenaError::ActivationNotReady.into());
    }

    accrue_rewards(&arena_config, &mut arena_position)?;
    let amount = arena_position.pending_activation_amount;
    arena_position.pending_activation_amount = 0;
    arena_position.eligible_amount = arena_position
        .eligible_amount
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_position.last_activity_ts = now;

    arena_config.pending_activation_locked = arena_config
        .pending_activation_locked
        .checked_sub(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_config.eligible_locked = arena_config
        .eligible_locked
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;

    arena_config.store(&mut config.data.borrow_mut())?;
    arena_position.store(&mut position.data.borrow_mut())
}

fn process_fund_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        return Err(ArenaError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    let funder = next_account_info(account_info_iter)?;
    let funder_token_account = next_account_info(account_info_iter)?;
    let config = next_account_info(account_info_iter)?;
    let reward_pool_token_account = next_account_info(account_info_iter)?;
    let mint = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;

    assert_signer(funder)?;
    assert_owned_by(config, program_id)?;
    assert_supported_token_program(token_program)?;

    let mut arena_config = ArenaConfig::load(&config.data.borrow())?;
    if arena_config.mint != *mint.key
        || arena_config.token_program != *token_program.key
        || arena_config.reward_pool_token_account != *reward_pool_token_account.key
    {
        return Err(ArenaError::InvalidTokenMint.into());
    }
    let (vault_authority, _) =
        Pubkey::find_program_address(&[VAULT_AUTHORITY_SEED, config.key.as_ref()], program_id);
    assert_token_account(
        token_program,
        funder_token_account,
        mint.key,
        Some(funder.key),
    )?;
    assert_token_account(
        token_program,
        reward_pool_token_account,
        mint.key,
        Some(&vault_authority),
    )?;

    let ix = transfer_checked_ix(
        token_program.key,
        funder_token_account.key,
        mint.key,
        reward_pool_token_account.key,
        funder.key,
        amount,
        arena_config.decimals,
    )?;
    invoke(
        &ix,
        &[
            funder_token_account.clone(),
            mint.clone(),
            reward_pool_token_account.clone(),
            funder.clone(),
            token_program.clone(),
        ],
    )?;

    arena_config.pending_rewards = arena_config
        .pending_rewards
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_config.total_rewards_funded = arena_config
        .total_rewards_funded
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_config.store(&mut config.data.borrow_mut())
}

fn process_roll_epoch(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config = next_account_info(account_info_iter)?;

    assert_owned_by(config, program_id)?;

    let mut arena_config = ArenaConfig::load(&config.data.borrow())?;
    let now = Clock::get()?.unix_timestamp;
    let next_epoch_ts = arena_config
        .last_epoch_ts
        .checked_add(arena_config.epoch_seconds)
        .ok_or(ArenaError::MathOverflow)?;
    if now < next_epoch_ts {
        return Err(ArenaError::EpochNotReady.into());
    }

    if arena_config.pending_rewards > 0 && arena_config.eligible_locked > 0 {
        let delta_index = u128::from(arena_config.pending_rewards)
            .checked_mul(REWARD_INDEX_SCALE)
            .ok_or(ArenaError::MathOverflow)?
            .checked_div(u128::from(arena_config.eligible_locked))
            .ok_or(ArenaError::MathOverflow)?;
        if delta_index > 0 {
            let distributed = delta_index
                .checked_mul(u128::from(arena_config.eligible_locked))
                .ok_or(ArenaError::MathOverflow)?
                .checked_div(REWARD_INDEX_SCALE)
                .ok_or(ArenaError::MathOverflow)?;
            let distributed = u64::try_from(distributed).map_err(|_| ArenaError::MathOverflow)?;
            arena_config.reward_index = arena_config
                .reward_index
                .checked_add(delta_index)
                .ok_or(ArenaError::MathOverflow)?;
            arena_config.pending_rewards = arena_config
                .pending_rewards
                .checked_sub(distributed)
                .ok_or(ArenaError::MathOverflow)?;
            arena_config.total_rewards_distributed = arena_config
                .total_rewards_distributed
                .checked_add(distributed)
                .ok_or(ArenaError::MathOverflow)?;
        }
    }

    arena_config.current_epoch = arena_config
        .current_epoch
        .checked_add(1)
        .ok_or(ArenaError::MathOverflow)?;
    arena_config.last_epoch_ts = now;
    arena_config.store(&mut config.data.borrow_mut())
}

#[allow(clippy::too_many_arguments)]
fn transfer_from_program_token_account<'a>(
    token_program: &AccountInfo<'a>,
    source: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    vault_authority: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    amount: u64,
    decimals: u8,
    config_key: &Pubkey,
    vault_authority_bump: u8,
) -> ProgramResult {
    if amount == 0 {
        return Ok(());
    }
    let ix = transfer_checked_ix(
        token_program.key,
        source.key,
        mint.key,
        destination.key,
        vault_authority.key,
        amount,
        decimals,
    )?;
    invoke_signed(
        &ix,
        &[
            source.clone(),
            mint.clone(),
            destination.clone(),
            vault_authority.clone(),
            token_program.clone(),
        ],
        &[&[
            VAULT_AUTHORITY_SEED,
            config_key.as_ref(),
            &[vault_authority_bump],
        ]],
    )
}

#[allow(clippy::too_many_arguments)]
fn burn_from_vault<'a>(
    token_program: &AccountInfo<'a>,
    vault_token_account: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    vault_authority: &AccountInfo<'a>,
    amount: u64,
    decimals: u8,
    config_key: &Pubkey,
    vault_authority_bump: u8,
) -> ProgramResult {
    if amount == 0 {
        return Ok(());
    }
    let ix = burn_checked_ix(
        token_program.key,
        vault_token_account.key,
        mint.key,
        vault_authority.key,
        amount,
        decimals,
    )?;
    invoke_signed(
        &ix,
        &[
            vault_token_account.clone(),
            mint.clone(),
            vault_authority.clone(),
            token_program.clone(),
        ],
        &[&[
            VAULT_AUTHORITY_SEED,
            config_key.as_ref(),
            &[vault_authority_bump],
        ]],
    )
}

fn process_claim_rewards(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let owner = next_account_info(account_info_iter)?;
    let owner_token_account = next_account_info(account_info_iter)?;
    let config = next_account_info(account_info_iter)?;
    let position = next_account_info(account_info_iter)?;
    let reward_pool_token_account = next_account_info(account_info_iter)?;
    let vault_authority = next_account_info(account_info_iter)?;
    let mint = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;

    assert_signer(owner)?;
    assert_owned_by(config, program_id)?;
    assert_owned_by(position, program_id)?;
    assert_supported_token_program(token_program)?;

    let mut arena_config = ArenaConfig::load(&config.data.borrow())?;
    if arena_config.mint != *mint.key
        || arena_config.token_program != *token_program.key
        || arena_config.reward_pool_token_account != *reward_pool_token_account.key
    {
        return Err(ArenaError::InvalidTokenMint.into());
    }
    let (expected_vault_authority, vault_authority_bump) =
        Pubkey::find_program_address(&[VAULT_AUTHORITY_SEED, config.key.as_ref()], program_id);
    if expected_vault_authority != *vault_authority.key {
        return Err(ArenaError::InvalidPda.into());
    }
    assert_token_account(
        token_program,
        owner_token_account,
        mint.key,
        Some(owner.key),
    )?;
    assert_token_account(
        token_program,
        reward_pool_token_account,
        mint.key,
        Some(vault_authority.key),
    )?;

    let mut arena_position = ArenaPosition::load(&position.data.borrow())?;
    if arena_position.config != *config.key || arena_position.owner != *owner.key {
        return Err(ArenaError::InvalidPda.into());
    }
    accrue_rewards(&arena_config, &mut arena_position)?;
    if arena_position.pending_rewards == 0 {
        return Err(ArenaError::NoRewards.into());
    }

    pay_pending_rewards(
        token_program,
        reward_pool_token_account,
        mint,
        vault_authority,
        owner_token_account,
        &mut arena_config,
        &mut arena_position,
        config.key,
        vault_authority_bump,
    )?;

    arena_position.last_activity_ts = Clock::get()?.unix_timestamp;

    arena_config.store(&mut config.data.borrow_mut())?;
    arena_position.store(&mut position.data.borrow_mut())
}

#[allow(clippy::too_many_arguments)]
fn pay_pending_rewards<'a>(
    token_program: &AccountInfo<'a>,
    reward_pool_token_account: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    vault_authority: &AccountInfo<'a>,
    owner_token_account: &AccountInfo<'a>,
    arena_config: &mut ArenaConfig,
    arena_position: &mut ArenaPosition,
    config_key: &Pubkey,
    vault_authority_bump: u8,
) -> ProgramResult {
    let amount = arena_position.pending_rewards;
    if amount == 0 {
        return Ok(());
    }

    transfer_from_program_token_account(
        token_program,
        reward_pool_token_account,
        mint,
        vault_authority,
        owner_token_account,
        amount,
        arena_config.decimals,
        config_key,
        vault_authority_bump,
    )?;

    arena_position.pending_rewards = 0;
    arena_position.total_rewards_claimed = arena_position
        .total_rewards_claimed
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_config.total_rewards_claimed = arena_config
        .total_rewards_claimed
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    Ok(())
}

fn penalty_split(
    amount: u64,
    early_exit_penalty_bps: u16,
    burn_penalty_bps: u16,
) -> Result<(u64, u64), ProgramError> {
    if early_exit_penalty_bps == 0 {
        return Ok((0, 0));
    }
    let penalty = amount
        .checked_mul(u64::from(early_exit_penalty_bps))
        .ok_or(ArenaError::MathOverflow)?
        .checked_add(BPS_DENOMINATOR - 1)
        .ok_or(ArenaError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(ArenaError::MathOverflow)?;
    let burn = if penalty > 0 && burn_penalty_bps > 0 {
        let burn = penalty
            .checked_mul(u64::from(burn_penalty_bps))
            .ok_or(ArenaError::MathOverflow)?
            .checked_div(u64::from(early_exit_penalty_bps))
            .ok_or(ArenaError::MathOverflow)?;
        burn.max(1).min(penalty)
    } else {
        0
    };
    let reward = penalty.checked_sub(burn).ok_or(ArenaError::MathOverflow)?;
    Ok((reward, burn))
}

fn reduce_position_stake(
    arena_config: &mut ArenaConfig,
    arena_position: &mut ArenaPosition,
    amount: u64,
) -> ProgramResult {
    let pending_reduction = arena_position.pending_activation_amount.min(amount);
    if pending_reduction > 0 {
        arena_position.pending_activation_amount = arena_position
            .pending_activation_amount
            .checked_sub(pending_reduction)
            .ok_or(ArenaError::MathOverflow)?;
        arena_config.pending_activation_locked = arena_config
            .pending_activation_locked
            .checked_sub(pending_reduction)
            .ok_or(ArenaError::MathOverflow)?;
    }

    let eligible_reduction = amount
        .checked_sub(pending_reduction)
        .ok_or(ArenaError::MathOverflow)?;
    if eligible_reduction > 0 {
        arena_position.eligible_amount = arena_position
            .eligible_amount
            .checked_sub(eligible_reduction)
            .ok_or(ArenaError::MathOverflow)?;
        arena_config.eligible_locked = arena_config
            .eligible_locked
            .checked_sub(eligible_reduction)
            .ok_or(ArenaError::MathOverflow)?;
    }

    arena_position.locked_amount = arena_position
        .locked_amount
        .checked_sub(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_config.total_locked = arena_config
        .total_locked
        .checked_sub(amount)
        .ok_or(ArenaError::MathOverflow)?;
    Ok(())
}

fn process_withdraw(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    if amount == 0 {
        return Err(ArenaError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    let owner = next_account_info(account_info_iter)?;
    let owner_token_account = next_account_info(account_info_iter)?;
    let config = next_account_info(account_info_iter)?;
    let position = next_account_info(account_info_iter)?;
    let vault_token_account = next_account_info(account_info_iter)?;
    let vault_authority = next_account_info(account_info_iter)?;
    let reward_pool_token_account = next_account_info(account_info_iter)?;
    let mint = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;

    assert_signer(owner)?;
    assert_owned_by(config, program_id)?;
    assert_owned_by(position, program_id)?;
    assert_supported_token_program(token_program)?;

    let mut arena_config = ArenaConfig::load(&config.data.borrow())?;
    if arena_config.mint != *mint.key
        || arena_config.token_program != *token_program.key
        || arena_config.vault_token_account != *vault_token_account.key
        || arena_config.reward_pool_token_account != *reward_pool_token_account.key
    {
        return Err(ArenaError::InvalidTokenMint.into());
    }
    if vault_token_account.key == reward_pool_token_account.key {
        return Err(ArenaError::InvalidTreasury.into());
    }

    let (expected_vault_authority, vault_authority_bump) =
        Pubkey::find_program_address(&[VAULT_AUTHORITY_SEED, config.key.as_ref()], program_id);
    if expected_vault_authority != *vault_authority.key {
        return Err(ArenaError::InvalidPda.into());
    }

    assert_token_account(
        token_program,
        owner_token_account,
        mint.key,
        Some(owner.key),
    )?;
    assert_token_account(
        token_program,
        vault_token_account,
        mint.key,
        Some(vault_authority.key),
    )?;
    assert_token_account(
        token_program,
        reward_pool_token_account,
        mint.key,
        Some(vault_authority.key),
    )?;

    let mut arena_position = ArenaPosition::load(&position.data.borrow())?;
    if arena_position.config != *config.key || arena_position.owner != *owner.key {
        return Err(ArenaError::InvalidPda.into());
    }
    if amount > arena_position.locked_amount {
        return Err(ArenaError::InsufficientPositionBalance.into());
    }
    let full_exit = amount == arena_position.locked_amount;

    accrue_rewards(&arena_config, &mut arena_position)?;
    if full_exit {
        pay_pending_rewards(
            token_program,
            reward_pool_token_account,
            mint,
            vault_authority,
            owner_token_account,
            &mut arena_config,
            &mut arena_position,
            config.key,
            vault_authority_bump,
        )?;
    }

    let now = Clock::get()?.unix_timestamp;
    let (reward_penalty, burned) =
        if now < arena_position.unlock_ts && arena_config.early_exit_penalty_bps > 0 {
            penalty_split(
                amount,
                arena_config.early_exit_penalty_bps,
                arena_config.burn_penalty_bps,
            )?
        } else {
            (0, 0)
        };
    let penalty = reward_penalty
        .checked_add(burned)
        .ok_or(ArenaError::MathOverflow)?;
    let returned = amount
        .checked_sub(penalty)
        .ok_or(ArenaError::MathOverflow)?;
    if returned == 0 {
        return Err(ArenaError::InvalidAmount.into());
    }

    transfer_from_program_token_account(
        token_program,
        vault_token_account,
        mint,
        vault_authority,
        owner_token_account,
        returned,
        arena_config.decimals,
        config.key,
        vault_authority_bump,
    )?;
    transfer_from_program_token_account(
        token_program,
        vault_token_account,
        mint,
        vault_authority,
        reward_pool_token_account,
        reward_penalty,
        arena_config.decimals,
        config.key,
        vault_authority_bump,
    )?;
    burn_from_vault(
        token_program,
        vault_token_account,
        mint,
        vault_authority,
        burned,
        arena_config.decimals,
        config.key,
        vault_authority_bump,
    )?;

    reduce_position_stake(&mut arena_config, &mut arena_position, amount)?;

    arena_position.total_withdrawn = arena_position
        .total_withdrawn
        .checked_add(returned)
        .ok_or(ArenaError::MathOverflow)?;
    arena_position.total_penalty_paid = arena_position
        .total_penalty_paid
        .checked_add(penalty)
        .ok_or(ArenaError::MathOverflow)?;
    arena_position.total_burned = arena_position
        .total_burned
        .checked_add(burned)
        .ok_or(ArenaError::MathOverflow)?;
    arena_position.last_activity_ts = now;

    arena_config.pending_rewards = arena_config
        .pending_rewards
        .checked_add(reward_penalty)
        .ok_or(ArenaError::MathOverflow)?;
    arena_config.total_penalties_collected = arena_config
        .total_penalties_collected
        .checked_add(penalty)
        .ok_or(ArenaError::MathOverflow)?;
    arena_config.total_burned = arena_config
        .total_burned
        .checked_add(burned)
        .ok_or(ArenaError::MathOverflow)?;

    arena_config.store(&mut config.data.borrow_mut())?;
    arena_position.store(&mut position.data.borrow_mut())
}
