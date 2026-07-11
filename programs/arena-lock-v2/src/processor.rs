use borsh::BorshDeserialize;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_option::COption,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction, system_program,
    sysvar::Sysvar,
};
use spl_token_2022::extension::{BaseStateWithExtensions, ExtensionType, StateWithExtensions};

use crate::{
    error::ArenaError,
    instruction::ArenaInstruction,
    state::{
        ArenaConfig, ArenaPosition, ARENA_CONFIG_VERSION, ARENA_POSITION_VERSION, BPS_DENOMINATOR,
        CONFIG_SEED, CONFIG_SIZE, MAX_ARENA_EARLY_EXIT_PENALTY_BPS, POSITION_SEED, POSITION_SIZE,
        REWARD_INDEX_SCALE, VAULT_AUTHORITY_SEED,
    },
};

/// M-05: lifetime telemetry must not hard-fail the program when saturating.
fn sat_add_u64(a: u64, b: u64) -> u64 {
    a.saturating_add(b)
}

/// Pump.fun and many meme mints use Token-2022 metadata extensions.
/// Allow only cosmetic / non-transfer-affecting mint extensions.
fn is_allowed_mint_extension(ext: ExtensionType) -> bool {
    matches!(
        ext,
        ExtensionType::MetadataPointer | ExtensionType::TokenMetadata
    )
}

/// ATAs often carry ImmutableOwner; reject transfer hooks / fee / permanent delegate.
fn is_allowed_account_extension(ext: ExtensionType) -> bool {
    matches!(ext, ExtensionType::ImmutableOwner)
}

fn assert_allowed_mint_extensions(exts: &[ExtensionType]) -> ProgramResult {
    for ext in exts {
        if !is_allowed_mint_extension(*ext) {
            return Err(ArenaError::InvalidTokenMint.into());
        }
    }
    Ok(())
}

fn assert_allowed_account_extensions(exts: &[ExtensionType]) -> ProgramResult {
    for ext in exts {
        if !is_allowed_account_extension(*ext) {
            return Err(ArenaError::InvalidTokenAccount.into());
        }
    }
    Ok(())
}

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
            process_fund_rewards(program_id, accounts, amount, None)
        }
        ArenaInstruction::FundRewardsChecked {
            amount,
            expected_eligible_locked,
            max_current_epoch,
        } => process_fund_rewards(
            program_id,
            accounts,
            amount,
            Some((expected_eligible_locked, max_current_epoch)),
        ),
        ArenaInstruction::RollEpoch => process_roll_epoch(program_id, accounts),
        ArenaInstruction::FinalizeRewards => process_finalize_rewards(program_id, accounts),
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

fn assert_writable(account: &AccountInfo) -> ProgramResult {
    if !account.is_writable {
        return Err(ArenaError::AccountNotWritable.into());
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
    amount: u64,
    is_initialized: bool,
    is_native: bool,
    is_frozen: bool,
    has_delegate: bool,
    delegated_amount: u64,
    has_close_authority: bool,
}

fn coption_is_some<T>(value: &COption<T>) -> bool {
    matches!(value, COption::Some(_))
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
            amount: token_account.amount,
            is_initialized: token_account.state == spl_token::state::AccountState::Initialized,
            is_native: coption_is_some(&token_account.is_native),
            is_frozen: token_account.is_frozen(),
            has_delegate: coption_is_some(&token_account.delegate),
            delegated_amount: token_account.delegated_amount,
            has_close_authority: coption_is_some(&token_account.close_authority),
        })
    } else if *token_program == spl_token_2022::id() {
        let account_data = account.data.borrow();
        let token_account =
            StateWithExtensions::<spl_token_2022::state::Account>::unpack(&account_data)
                .map_err(|_| ArenaError::InvalidTokenAccount)?;
        let exts = token_account
            .get_extension_types()
            .map_err(|_| ArenaError::InvalidTokenAccount)?;
        assert_allowed_account_extensions(&exts)?;
        Ok(TokenAccountView {
            mint: token_account.base.mint,
            owner: token_account.base.owner,
            amount: token_account.base.amount,
            is_initialized: token_account.base.state
                == spl_token_2022::state::AccountState::Initialized,
            is_native: coption_is_some(&token_account.base.is_native),
            is_frozen: token_account.base.is_frozen(),
            has_delegate: coption_is_some(&token_account.base.delegate),
            delegated_amount: token_account.base.delegated_amount,
            has_close_authority: coption_is_some(&token_account.base.close_authority),
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
        let mint_state = spl_token::state::Mint::unpack(&mint.data.borrow())
            .map_err(|_| ArenaError::InvalidTokenMint)?;
        if coption_is_some(&mint_state.mint_authority) {
            return Err(ArenaError::InvalidTokenMint.into());
        }
        if mint_state.freeze_authority.is_some() {
            return Err(ArenaError::InvalidTokenMint.into());
        }
        Ok(mint_state.decimals)
    } else if *token_program == spl_token_2022::id() {
        let mint_data = mint.data.borrow();
        let mint_state = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)
            .map_err(|_| ArenaError::InvalidTokenMint)?;
        let exts = mint_state
            .get_extension_types()
            .map_err(|_| ArenaError::InvalidTokenMint)?;
        // Pump.fun: MetadataPointer + TokenMetadata OK; hooks/fees/permanent-delegate rejected.
        assert_allowed_mint_extensions(&exts)?;
        if coption_is_some(&mint_state.base.mint_authority) {
            return Err(ArenaError::InvalidTokenMint.into());
        }
        if mint_state.base.freeze_authority.is_some() {
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

fn assert_custody_token_account(
    token_program: &AccountInfo,
    account: &AccountInfo,
    mint: &Pubkey,
    owner: &Pubkey,
) -> ProgramResult {
    let view = unpack_token_account(token_program.key, account)?;
    if view.mint != *mint {
        return Err(ArenaError::InvalidTokenMint.into());
    }
    if view.owner != *owner
        || !view.is_initialized
        || view.is_native
        || view.is_frozen
        || view.has_delegate
        || view.delegated_amount != 0
        || view.has_close_authority
    {
        return Err(ArenaError::InvalidTokenAccount.into());
    }
    Ok(())
}

fn assert_config_pda(
    program_id: &Pubkey,
    config_account: &AccountInfo,
    config: &ArenaConfig,
) -> ProgramResult {
    let (expected_config, _) = Pubkey::find_program_address(
        &[
            CONFIG_SEED,
            config.authority.as_ref(),
            &config.config_id.to_le_bytes(),
        ],
        program_id,
    );
    if expected_config != *config_account.key {
        return Err(ArenaError::InvalidPda.into());
    }
    Ok(())
}

fn assert_position_pda(
    program_id: &Pubkey,
    position_account: &AccountInfo,
    config: &Pubkey,
    owner: &Pubkey,
) -> ProgramResult {
    let (expected_position, _) = Pubkey::find_program_address(
        &[POSITION_SEED, config.as_ref(), owner.as_ref()],
        program_id,
    );
    if expected_position != *position_account.key {
        return Err(ArenaError::InvalidPda.into());
    }
    Ok(())
}

fn reward_index_delta(amount: u64, eligible: u64) -> Result<u128, ProgramError> {
    if amount == 0 || eligible == 0 {
        return Err(ArenaError::InvalidAmount.into());
    }
    let delta = u128::from(amount)
        .checked_mul(REWARD_INDEX_SCALE)
        .ok_or(ArenaError::MathOverflow)?
        .checked_div(u128::from(eligible))
        .ok_or(ArenaError::MathOverflow)?;
    if delta == 0 {
        return Err(ArenaError::InvalidAmount.into());
    }
    Ok(delta)
}

fn add_reward_index(config: &mut ArenaConfig, delta: u128) -> ProgramResult {
    let (next, wrapped) = config.reward_index.overflowing_add(delta);
    config.reward_index = next;
    if wrapped {
        config.reward_index_generation = config
            .reward_index_generation
            .checked_add(1)
            .ok_or(ArenaError::MathOverflow)?;
    }
    Ok(())
}

fn commit_rewards_to_index(
    config: &mut ArenaConfig,
    amount: u64,
    eligible_snapshot: u64,
) -> ProgramResult {
    let delta = reward_index_delta(amount, eligible_snapshot)?;
    add_reward_index(config, delta)?;
    config.total_rewards_distributed = sat_add_u64(config.total_rewards_distributed, amount);
    Ok(())
}

fn try_commit_rewards_to_index(
    config: &mut ArenaConfig,
    amount: u64,
    eligible_snapshot: u64,
) -> ProgramResult {
    if amount == 0 || eligible_snapshot == 0 {
        return Ok(());
    }
    if let Ok(delta) = reward_index_delta(amount, eligible_snapshot) {
        add_reward_index(config, delta)?;
        config.total_rewards_distributed = sat_add_u64(config.total_rewards_distributed, amount);
    }
    Ok(())
}

fn reward_index_delta_since(
    config: &ArenaConfig,
    position: &ArenaPosition,
) -> Result<u128, ProgramError> {
    if config.reward_index_generation == position.reward_index_generation_checkpoint {
        if config.reward_index < position.reward_index_checkpoint {
            return Err(ArenaError::MathOverflow.into());
        }
        return config
            .reward_index
            .checked_sub(position.reward_index_checkpoint)
            .ok_or(ArenaError::MathOverflow.into());
    }

    let generation_delta = config
        .reward_index_generation
        .checked_sub(position.reward_index_generation_checkpoint)
        .ok_or(ArenaError::MathOverflow)?;
    if generation_delta != 1 {
        return Err(ArenaError::MathOverflow.into());
    }

    let tail = u128::MAX
        .checked_sub(position.reward_index_checkpoint)
        .ok_or(ArenaError::MathOverflow)?;
    let tail = tail.checked_add(1).ok_or(ArenaError::MathOverflow)?;
    tail.checked_add(config.reward_index)
        .ok_or(ArenaError::MathOverflow.into())
}

/// H-02: promote warming stake to mature eligible after at least one epoch
/// boundary has passed since activation (`current_epoch > warming_epoch`).
fn mature_warming_if_ready(
    config: &mut ArenaConfig,
    position: &mut ArenaPosition,
) -> ProgramResult {
    if position.warming_amount == 0 {
        return Ok(());
    }
    if config.current_epoch <= position.warming_epoch {
        return Ok(());
    }
    accrue_rewards(config, position)?;
    let amount = position.warming_amount;
    position.warming_amount = 0;
    position.eligible_amount = position
        .eligible_amount
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    config.warming_locked = config
        .warming_locked
        .checked_sub(amount)
        .ok_or(ArenaError::MathOverflow)?;
    config.eligible_locked = config
        .eligible_locked
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    // New mature stake starts after all rewards already indexed at promotion time.
    position.reward_index_checkpoint = config.reward_index;
    position.reward_index_generation_checkpoint = config.reward_index_generation;
    Ok(())
}

fn accrue_rewards(config: &ArenaConfig, position: &mut ArenaPosition) -> ProgramResult {
    let delta = reward_index_delta_since(config, position)?;
    if delta > 0 && position.eligible_amount > 0 {
        // M-04: carry remainder so multi-user floor dust is minimized over time.
        let scaled = u128::from(position.eligible_amount)
            .checked_mul(delta)
            .ok_or(ArenaError::MathOverflow)?
            .checked_add(position.reward_accrual_remainder)
            .ok_or(ArenaError::MathOverflow)?;
        let earned = scaled
            .checked_div(REWARD_INDEX_SCALE)
            .ok_or(ArenaError::MathOverflow)?;
        position.reward_accrual_remainder = scaled
            .checked_rem(REWARD_INDEX_SCALE)
            .ok_or(ArenaError::MathOverflow)?;
        let earned = u64::try_from(earned).map_err(|_| ArenaError::MathOverflow)?;
        position.pending_rewards = position
            .pending_rewards
            .checked_add(earned)
            .ok_or(ArenaError::MathOverflow)?;
    }
    position.reward_index_checkpoint = config.reward_index;
    position.reward_index_generation_checkpoint = config.reward_index_generation;
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
    if early_exit_penalty_bps > 0
        && u128::from(min_deposit_amount)
            .checked_mul(u128::from(early_exit_penalty_bps))
            .ok_or(ArenaError::MathOverflow)?
            < u128::from(BPS_DENOMINATOR)
    {
        return Err(ArenaError::InvalidPenalty.into());
    }
    if burn_penalty_bps > 0
        && u128::from(min_deposit_amount)
            .checked_mul(u128::from(burn_penalty_bps))
            .ok_or(ArenaError::MathOverflow)?
            < u128::from(BPS_DENOMINATOR)
    {
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
    assert_writable(authority)?;
    assert_writable(config)?;
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
    assert_custody_token_account(
        token_program,
        vault_token_account,
        mint.key,
        vault_authority.key,
    )?;
    assert_custody_token_account(
        token_program,
        reward_pool_token_account,
        mint.key,
        vault_authority.key,
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
        version: ARENA_CONFIG_VERSION,
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
        reward_index_generation: 0,
        total_rewards_expired: 0,
        reward_dust: 0,
        warming_locked: 0,
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
    assert_writable(owner)?;
    assert_writable(owner_token_account)?;
    assert_writable(config)?;
    assert_writable(position)?;
    assert_writable(vault_token_account)?;
    assert_owned_by(config, program_id)?;
    assert_supported_token_program(token_program)?;
    assert_system_program(system_program)?;

    let mut arena_config = ArenaConfig::load(&config.data.borrow())?;
    assert_config_pda(program_id, config, &arena_config)?;
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
    assert_custody_token_account(
        token_program,
        vault_token_account,
        mint.key,
        vault_authority.key,
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
        mature_warming_if_ready(&mut arena_config, &mut existing)?;
        accrue_rewards(&arena_config, &mut existing)?;
        existing
    } else {
        ArenaPosition {
            is_initialized: true,
            version: ARENA_POSITION_VERSION,
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
            reward_index_generation_checkpoint: arena_config.reward_index_generation,
            position_bump,
            warming_amount: 0,
            warming_epoch: 0,
            penalty_remainder: 0,
            burn_remainder: 0,
            reward_accrual_remainder: 0,
        }
    };

    // H1: do not reset the whole position clock on top-ups.
    // - first stake sets lock_start / unlock / activation from now
    // - later deposits only extend unlock if needed, and only delay activation for new pending
    let had_locked = arena_position.locked_amount > 0;
    let prior_pending = arena_position.pending_activation_amount;
    let min_unlock = now
        .checked_add(arena_config.min_lock_seconds)
        .ok_or(ArenaError::MathOverflow)?;
    let min_activation = now
        .checked_add(arena_config.activation_delay_seconds)
        .ok_or(ArenaError::MathOverflow)?;

    arena_position.locked_amount = arena_position
        .locked_amount
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_position.pending_activation_amount = arena_position
        .pending_activation_amount
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_position.total_deposited = sat_add_u64(arena_position.total_deposited, amount);

    if !had_locked {
        arena_position.lock_start_ts = now;
        arena_position.unlock_ts = min_unlock;
        arena_position.activation_ts = min_activation;
    } else {
        if min_unlock > arena_position.unlock_ts {
            arena_position.unlock_ts = min_unlock;
        }
        if prior_pending == 0 {
            // Fresh pending slice after fully eligible stake
            arena_position.activation_ts = min_activation;
        } else if min_activation > arena_position.activation_ts {
            // Pending activates as one bag — delay to cover the newest deposit
            arena_position.activation_ts = min_activation;
        }
    }
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
    // M2: always present so orphaned pending pool can be burned before first eligibility
    let reward_pool_token_account = next_account_info(account_info_iter)?;
    let vault_authority = next_account_info(account_info_iter)?;
    let mint = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;

    assert_signer(owner)?;
    assert_writable(config)?;
    assert_writable(position)?;
    assert_owned_by(config, program_id)?;
    assert_owned_by(position, program_id)?;
    assert_supported_token_program(token_program)?;

    let mut arena_config = ArenaConfig::load(&config.data.borrow())?;
    assert_config_pda(program_id, config, &arena_config)?;
    if arena_config.mint != *mint.key
        || arena_config.token_program != *token_program.key
        || arena_config.reward_pool_token_account != *reward_pool_token_account.key
    {
        return Err(ArenaError::InvalidTokenMint.into());
    }
    let (expected_vault_authority, _) =
        Pubkey::find_program_address(&[VAULT_AUTHORITY_SEED, config.key.as_ref()], program_id);
    if expected_vault_authority != *vault_authority.key {
        return Err(ArenaError::InvalidPda.into());
    }
    assert_custody_token_account(
        token_program,
        reward_pool_token_account,
        mint.key,
        vault_authority.key,
    )?;

    let mut arena_position = ArenaPosition::load(&position.data.borrow())?;
    if arena_position.config != *config.key || arena_position.owner != *owner.key {
        return Err(ArenaError::InvalidPda.into());
    }
    assert_position_pda(program_id, position, config.key, owner.key)?;
    // Always allow maturity sync even with zero pending (post-epoch touch).
    mature_warming_if_ready(&mut arena_config, &mut arena_position)?;
    if arena_position.pending_activation_amount == 0 {
        // Maturity-only sync path: persist and return success so clients can
        // promote warming without depositing more.
        if arena_position.warming_amount == 0
            && arena_position.eligible_amount == 0
            && arena_position.locked_amount == 0
        {
            return Err(ArenaError::InvalidAmount.into());
        }
        accrue_rewards(&arena_config, &mut arena_position)?;
        arena_position.last_activity_ts = Clock::get()?.unix_timestamp;
        arena_config.store(&mut config.data.borrow_mut())?;
        arena_position.store(&mut position.data.borrow_mut())?;
        return Ok(());
    }
    let now = Clock::get()?.unix_timestamp;
    if now < arena_position.activation_ts {
        return Err(ArenaError::ActivationNotReady.into());
    }

    accrue_rewards(&arena_config, &mut arena_position)?;
    // H-02: activation enters warming until the next epoch boundary passes.
    // Stake is excluded from RollEpoch distribution until matured.
    let amount = arena_position.pending_activation_amount;
    arena_position.pending_activation_amount = 0;
    // Keep the earliest warming epoch so partial tops-ups don't delay maturity.
    if arena_position.warming_amount == 0 {
        arena_position.warming_epoch = arena_config.current_epoch;
    }
    arena_position.warming_amount = arena_position
        .warming_amount
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_position.last_activity_ts = now;

    arena_config.pending_activation_locked = arena_config
        .pending_activation_locked
        .checked_sub(amount)
        .ok_or(ArenaError::MathOverflow)?;
    arena_config.warming_locked = arena_config
        .warming_locked
        .checked_add(amount)
        .ok_or(ArenaError::MathOverflow)?;

    arena_config.store(&mut config.data.borrow_mut())?;
    arena_position.store(&mut position.data.borrow_mut())
}

fn process_fund_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
    expected_snapshot: Option<(u64, u64)>,
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
    assert_writable(funder_token_account)?;
    assert_writable(config)?;
    assert_writable(reward_pool_token_account)?;
    assert_owned_by(config, program_id)?;
    assert_supported_token_program(token_program)?;

    let mut arena_config = ArenaConfig::load(&config.data.borrow())?;
    assert_config_pda(program_id, config, &arena_config)?;
    if arena_config.mint != *mint.key
        || arena_config.token_program != *token_program.key
        || arena_config.reward_pool_token_account != *reward_pool_token_account.key
    {
        return Err(ArenaError::InvalidTokenMint.into());
    }
    if arena_config.eligible_locked == 0 {
        return Err(ArenaError::NoEligibleStake.into());
    }
    let eligible_snapshot = arena_config.eligible_locked;
    if let Some((expected_eligible_locked, max_current_epoch)) = expected_snapshot {
        if eligible_snapshot != expected_eligible_locked
            || arena_config.current_epoch > max_current_epoch
        {
            return Err(ArenaError::FundingSnapshotMismatch.into());
        }
    }
    reward_index_delta(amount, eligible_snapshot)?;
    let (vault_authority, _) =
        Pubkey::find_program_address(&[VAULT_AUTHORITY_SEED, config.key.as_ref()], program_id);
    assert_token_account(
        token_program,
        funder_token_account,
        mint.key,
        Some(funder.key),
    )?;
    assert_custody_token_account(
        token_program,
        reward_pool_token_account,
        mint.key,
        &vault_authority,
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

    commit_rewards_to_index(&mut arena_config, amount, eligible_snapshot)?;
    arena_config.total_rewards_funded = sat_add_u64(arena_config.total_rewards_funded, amount);
    arena_config.store(&mut config.data.borrow_mut())
}

fn process_roll_epoch(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config = next_account_info(account_info_iter)?;
    // M2: burn path accounts when empty arena still has pending pool tokens
    let reward_pool_token_account = next_account_info(account_info_iter)?;
    let vault_authority = next_account_info(account_info_iter)?;
    let mint = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;

    assert_writable(config)?;
    assert_owned_by(config, program_id)?;
    assert_supported_token_program(token_program)?;

    let mut arena_config = ArenaConfig::load(&config.data.borrow())?;
    assert_config_pda(program_id, config, &arena_config)?;
    if arena_config.mint != *mint.key
        || arena_config.token_program != *token_program.key
        || arena_config.reward_pool_token_account != *reward_pool_token_account.key
    {
        return Err(ArenaError::InvalidTokenMint.into());
    }
    let (expected_vault_authority, _) =
        Pubkey::find_program_address(&[VAULT_AUTHORITY_SEED, config.key.as_ref()], program_id);
    if expected_vault_authority != *vault_authority.key {
        return Err(ArenaError::InvalidPda.into());
    }
    assert_custody_token_account(
        token_program,
        reward_pool_token_account,
        mint.key,
        vault_authority.key,
    )?;

    let now = Clock::get()?.unix_timestamp;
    let next_epoch_ts = arena_config
        .last_epoch_ts
        .checked_add(arena_config.epoch_seconds)
        .ok_or(ArenaError::MathOverflow)?;
    if now < next_epoch_ts {
        return Err(ArenaError::EpochNotReady.into());
    }

    // Rewards are indexed at funding time. Rolling only advances the admission
    // epoch, so a same-transaction position touch cannot alter a funded batch's
    // denominator.
    arena_config.current_epoch = arena_config
        .current_epoch
        .checked_add(1)
        .ok_or(ArenaError::MathOverflow)?;
    arena_config.last_epoch_ts = next_epoch_ts;
    arena_config.store(&mut config.data.borrow_mut())
}

fn process_finalize_rewards(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config = next_account_info(account_info_iter)?;
    let reward_pool_token_account = next_account_info(account_info_iter)?;
    let vault_authority = next_account_info(account_info_iter)?;
    let mint = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;

    assert_writable(config)?;
    assert_writable(reward_pool_token_account)?;
    assert_writable(mint)?;
    assert_owned_by(config, program_id)?;
    assert_supported_token_program(token_program)?;

    let mut arena_config = ArenaConfig::load(&config.data.borrow())?;
    assert_config_pda(program_id, config, &arena_config)?;
    if arena_config.mint != *mint.key
        || arena_config.token_program != *token_program.key
        || arena_config.reward_pool_token_account != *reward_pool_token_account.key
    {
        return Err(ArenaError::InvalidTokenMint.into());
    }
    if arena_config.total_locked != 0
        || arena_config.eligible_locked != 0
        || arena_config.warming_locked != 0
        || arena_config.pending_activation_locked != 0
        || arena_config.pending_rewards != 0
    {
        return Err(ArenaError::InvalidAmount.into());
    }

    let (expected_vault_authority, vault_authority_bump) =
        Pubkey::find_program_address(&[VAULT_AUTHORITY_SEED, config.key.as_ref()], program_id);
    if expected_vault_authority != *vault_authority.key {
        return Err(ArenaError::InvalidPda.into());
    }
    assert_custody_token_account(
        token_program,
        reward_pool_token_account,
        mint.key,
        vault_authority.key,
    )?;
    burn_reward_pool_balance(
        token_program,
        reward_pool_token_account,
        mint,
        vault_authority,
        &mut arena_config,
        config.key,
        vault_authority_bump,
    )?;
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

/// Burn the actual reward-pool token balance once the arena has no stake/debt.
#[allow(clippy::too_many_arguments)]
fn burn_reward_pool_balance<'a>(
    token_program: &AccountInfo<'a>,
    reward_pool_token_account: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    vault_authority: &AccountInfo<'a>,
    arena_config: &mut ArenaConfig,
    config_key: &Pubkey,
    vault_authority_bump: u8,
) -> ProgramResult {
    let view = unpack_token_account(token_program.key, reward_pool_token_account)?;
    let amount = view.amount;
    if amount == 0 {
        return Ok(());
    }

    burn_from_vault(
        token_program,
        reward_pool_token_account,
        mint,
        vault_authority,
        amount,
        arena_config.decimals,
        config_key,
        vault_authority_bump,
    )?;
    arena_config.pending_rewards = 0;
    arena_config.reward_dust = 0;
    arena_config.total_rewards_expired = sat_add_u64(arena_config.total_rewards_expired, amount);
    arena_config.total_burned = sat_add_u64(arena_config.total_burned, amount);
    Ok(())
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
    assert_writable(owner_token_account)?;
    assert_writable(config)?;
    assert_writable(position)?;
    assert_writable(reward_pool_token_account)?;
    assert_owned_by(config, program_id)?;
    assert_owned_by(position, program_id)?;
    assert_supported_token_program(token_program)?;

    let mut arena_config = ArenaConfig::load(&config.data.borrow())?;
    assert_config_pda(program_id, config, &arena_config)?;
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
    assert_custody_token_account(
        token_program,
        reward_pool_token_account,
        mint.key,
        vault_authority.key,
    )?;

    let mut arena_position = ArenaPosition::load(&position.data.borrow())?;
    if arena_position.config != *config.key || arena_position.owner != *owner.key {
        return Err(ArenaError::InvalidPda.into());
    }
    assert_position_pda(program_id, position, config.key, owner.key)?;
    if arena_position.locked_amount == 0 {
        return Err(ArenaError::InsufficientPositionBalance.into());
    }
    let maturity_sync = arena_position.warming_amount > 0
        && arena_config.current_epoch > arena_position.warming_epoch;
    mature_warming_if_ready(&mut arena_config, &mut arena_position)?;
    accrue_rewards(&arena_config, &mut arena_position)?;
    if arena_position.pending_rewards == 0 {
        if maturity_sync {
            arena_position.last_activity_ts = Clock::get()?.unix_timestamp;
            arena_config.store(&mut config.data.borrow_mut())?;
            arena_position.store(&mut position.data.borrow_mut())?;
            return Ok(());
        }
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
    arena_position.total_rewards_claimed =
        sat_add_u64(arena_position.total_rewards_claimed, amount);
    arena_config.total_rewards_claimed = sat_add_u64(arena_config.total_rewards_claimed, amount);
    Ok(())
}

/// H-03: cumulative early-exit penalty with remainder so split withdraws cannot
/// bypass the bps floor (Withdraw(2) at 50% == two Withdraw(1) over time).
fn penalty_split(
    amount: u64,
    early_exit_penalty_bps: u16,
    burn_penalty_bps: u16,
    penalty_remainder: &mut u64,
    burn_remainder: &mut u64,
) -> Result<(u64, u64), ProgramError> {
    if early_exit_penalty_bps == 0 {
        return Ok((0, 0));
    }
    let scaled = u128::from(amount)
        .checked_mul(u128::from(early_exit_penalty_bps))
        .ok_or(ArenaError::MathOverflow)?
        .checked_add(u128::from(*penalty_remainder))
        .ok_or(ArenaError::MathOverflow)?;
    let penalty = u64::try_from(
        scaled
            .checked_div(u128::from(BPS_DENOMINATOR))
            .ok_or(ArenaError::MathOverflow)?,
    )
    .map_err(|_| ArenaError::MathOverflow)?;
    *penalty_remainder = u64::try_from(
        scaled
            .checked_rem(u128::from(BPS_DENOMINATOR))
            .ok_or(ArenaError::MathOverflow)?,
    )
    .map_err(|_| ArenaError::MathOverflow)?;

    // Never confiscate more than the withdrawn amount.
    let penalty = penalty.min(amount);

    let burn = if burn_penalty_bps > 0 {
        let scaled_burn = u128::from(amount)
            .checked_mul(u128::from(burn_penalty_bps))
            .ok_or(ArenaError::MathOverflow)?
            .checked_add(u128::from(*burn_remainder))
            .ok_or(ArenaError::MathOverflow)?;
        let burn = u64::try_from(
            scaled_burn
                .checked_div(u128::from(BPS_DENOMINATOR))
                .ok_or(ArenaError::MathOverflow)?,
        )
        .map_err(|_| ArenaError::MathOverflow)?;
        *burn_remainder = u64::try_from(
            scaled_burn
                .checked_rem(u128::from(BPS_DENOMINATOR))
                .ok_or(ArenaError::MathOverflow)?,
        )
        .map_err(|_| ArenaError::MathOverflow)?;
        burn.min(penalty)
    } else {
        *burn_remainder = 0;
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
    // Order: pending activation → warming → mature eligible.
    let mut remaining = amount;

    let pending_reduction = arena_position.pending_activation_amount.min(remaining);
    if pending_reduction > 0 {
        arena_position.pending_activation_amount = arena_position
            .pending_activation_amount
            .checked_sub(pending_reduction)
            .ok_or(ArenaError::MathOverflow)?;
        arena_config.pending_activation_locked = arena_config
            .pending_activation_locked
            .checked_sub(pending_reduction)
            .ok_or(ArenaError::MathOverflow)?;
        remaining = remaining
            .checked_sub(pending_reduction)
            .ok_or(ArenaError::MathOverflow)?;
    }

    let warming_reduction = arena_position.warming_amount.min(remaining);
    if warming_reduction > 0 {
        arena_position.warming_amount = arena_position
            .warming_amount
            .checked_sub(warming_reduction)
            .ok_or(ArenaError::MathOverflow)?;
        arena_config.warming_locked = arena_config
            .warming_locked
            .checked_sub(warming_reduction)
            .ok_or(ArenaError::MathOverflow)?;
        remaining = remaining
            .checked_sub(warming_reduction)
            .ok_or(ArenaError::MathOverflow)?;
    }

    if remaining > 0 {
        arena_position.eligible_amount = arena_position
            .eligible_amount
            .checked_sub(remaining)
            .ok_or(ArenaError::MathOverflow)?;
        arena_config.eligible_locked = arena_config
            .eligible_locked
            .checked_sub(remaining)
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

    if arena_position.locked_amount == 0 {
        arena_position.reward_accrual_remainder = 0;
    }
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
    assert_writable(owner_token_account)?;
    assert_writable(config)?;
    assert_writable(position)?;
    assert_writable(vault_token_account)?;
    assert_writable(reward_pool_token_account)?;
    assert_writable(mint)?;
    assert_owned_by(config, program_id)?;
    assert_owned_by(position, program_id)?;
    assert_supported_token_program(token_program)?;

    let mut arena_config = ArenaConfig::load(&config.data.borrow())?;
    assert_config_pda(program_id, config, &arena_config)?;
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
    assert_custody_token_account(
        token_program,
        vault_token_account,
        mint.key,
        vault_authority.key,
    )?;
    assert_custody_token_account(
        token_program,
        reward_pool_token_account,
        mint.key,
        vault_authority.key,
    )?;

    let mut arena_position = ArenaPosition::load(&position.data.borrow())?;
    if arena_position.config != *config.key || arena_position.owner != *owner.key {
        return Err(ArenaError::InvalidPda.into());
    }
    assert_position_pda(program_id, position, config.key, owner.key)?;
    if amount > arena_position.locked_amount {
        return Err(ArenaError::InsufficientPositionBalance.into());
    }
    mature_warming_if_ready(&mut arena_config, &mut arena_position)?;

    let full_exit = amount == arena_position.locked_amount;
    // Preview stake-bucket reductions (pending → warming → eligible).
    let pending_reduction = arena_position.pending_activation_amount.min(amount);
    let after_pending = amount
        .checked_sub(pending_reduction)
        .ok_or(ArenaError::MathOverflow)?;
    let warming_reduction = arena_position.warming_amount.min(after_pending);
    let eligible_reduction = after_pending
        .checked_sub(warming_reduction)
        .ok_or(ArenaError::MathOverflow)?;
    let eligible_locked_after = arena_config
        .eligible_locked
        .checked_sub(eligible_reduction)
        .ok_or(ArenaError::MathOverflow)?;
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
    let (mut reward_penalty, mut burned) =
        if now < arena_position.unlock_ts && arena_config.early_exit_penalty_bps > 0 {
            penalty_split(
                amount,
                arena_config.early_exit_penalty_bps,
                arena_config.burn_penalty_bps,
                &mut arena_position.penalty_remainder,
                &mut arena_position.burn_remainder,
            )?
        } else {
            // Past unlock: clear remainder so it cannot apply after lock ends.
            arena_position.penalty_remainder = 0;
            arena_position.burn_remainder = 0;
            (0, 0)
        };
    // No mature denominator remains, so redistribution would be snipeable.
    if eligible_locked_after == 0 {
        burned = burned
            .checked_add(reward_penalty)
            .ok_or(ArenaError::MathOverflow)?;
        reward_penalty = 0;
    }
    let penalty = reward_penalty
        .checked_add(burned)
        .ok_or(ArenaError::MathOverflow)?;
    let returned = amount
        .checked_sub(penalty)
        .ok_or(ArenaError::MathOverflow)?;
    // Allow full confiscation when cumulative penalty equals the withdrawn amount
    // (H-03 remainder catch-up on tiny splits). Reject only the zero-withdraw case
    // where neither principal nor penalty moves value.
    if returned == 0 && penalty == 0 {
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
    try_commit_rewards_to_index(&mut arena_config, reward_penalty, eligible_locked_after)?;

    reduce_position_stake(&mut arena_config, &mut arena_position, amount)?;

    arena_position.total_withdrawn = sat_add_u64(arena_position.total_withdrawn, returned);
    arena_position.total_penalty_paid = sat_add_u64(arena_position.total_penalty_paid, penalty);
    arena_position.total_burned = sat_add_u64(arena_position.total_burned, burned);
    arena_position.last_activity_ts = now;

    arena_config.total_penalties_collected =
        sat_add_u64(arena_config.total_penalties_collected, penalty);
    arena_config.total_burned = sat_add_u64(arena_config.total_burned, burned);

    arena_config.store(&mut config.data.borrow_mut())?;
    arena_position.store(&mut position.data.borrow_mut())
}

#[cfg(test)]
mod unit_tests {
    use super::penalty_split;

    #[test]
    fn cumulative_penalty_matches_bulk_for_fifty_percent() {
        let mut rem = 0u64;
        let mut burn_rem = 0u64;
        let (r1, b1) = penalty_split(1, 5_000, 0, &mut rem, &mut burn_rem).unwrap();
        assert_eq!((r1, b1, rem), (0, 0, 5_000));
        let (r2, b2) = penalty_split(1, 5_000, 0, &mut rem, &mut burn_rem).unwrap();
        assert_eq!((r2, b2, rem), (1, 0, 0));

        let mut rem_bulk = 0u64;
        let mut burn_rem_bulk = 0u64;
        let (rb, bb) = penalty_split(2, 5_000, 0, &mut rem_bulk, &mut burn_rem_bulk).unwrap();
        assert_eq!((rb, bb), (1, 0));
        assert_eq!(r1 + r2, rb);
    }

    #[test]
    fn cumulative_burn_matches_bulk_for_small_splits() {
        let mut penalty_rem = 0u64;
        let mut burn_rem = 0u64;
        let mut reward_total = 0u64;
        let mut burn_total = 0u64;
        for _ in 0..10 {
            let (reward, burn) =
                penalty_split(10, 1_000, 100, &mut penalty_rem, &mut burn_rem).unwrap();
            reward_total += reward;
            burn_total += burn;
        }

        let mut bulk_penalty_rem = 0u64;
        let mut bulk_burn_rem = 0u64;
        let (bulk_reward, bulk_burn) =
            penalty_split(100, 1_000, 100, &mut bulk_penalty_rem, &mut bulk_burn_rem).unwrap();
        assert_eq!((reward_total, burn_total), (bulk_reward, bulk_burn));
        assert_eq!((bulk_reward, bulk_burn), (9, 1));
    }
}
