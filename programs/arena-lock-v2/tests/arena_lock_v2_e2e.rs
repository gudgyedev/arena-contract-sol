#![allow(deprecated)]

use solana_program::program_pack::Pack;
use solana_program_test::{processor, ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::{Account, AccountSharedData},
    clock::Clock,
    instruction::Instruction,
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};

use arena_lock_v2::{
    id,
    instruction::{
        activate_position, claim_rewards, config_pda, deposit, finalize_rewards, fund_rewards,
        fund_rewards_checked, initialize_config, position_pda, roll_epoch, vault_authority_pda,
        withdraw,
    },
    processor::process_instruction,
    state::{ArenaConfig, ArenaPosition, ARENA_POSITION_VERSION, CONFIG_SIZE, POSITION_SIZE},
};

const DECIMALS: u8 = 6;
const USER_STARTING_TOKENS: u64 = 10_000_000;

#[derive(Clone, Copy)]
enum TokenFlavor {
    Spl,
    Token2022,
}

impl TokenFlavor {
    fn program_id(self) -> Pubkey {
        match self {
            Self::Spl => spl_token::id(),
            Self::Token2022 => spl_token_2022::id(),
        }
    }

    fn mint_len(self) -> usize {
        match self {
            Self::Spl => spl_token::state::Mint::LEN,
            Self::Token2022 => spl_token_2022::state::Mint::LEN,
        }
    }

    fn account_len(self) -> usize {
        match self {
            Self::Spl => spl_token::state::Account::LEN,
            Self::Token2022 => spl_token_2022::state::Account::LEN,
        }
    }

    fn initialize_mint(self, mint: &Pubkey, authority: &Pubkey, decimals: u8) -> Instruction {
        match self {
            Self::Spl => spl_token::instruction::initialize_mint(
                &self.program_id(),
                mint,
                authority,
                None,
                decimals,
            )
            .unwrap(),
            Self::Token2022 => spl_token_2022::instruction::initialize_mint(
                &self.program_id(),
                mint,
                authority,
                None,
                decimals,
            )
            .unwrap(),
        }
    }

    fn initialize_account(self, account: &Pubkey, mint: &Pubkey, owner: &Pubkey) -> Instruction {
        match self {
            Self::Spl => {
                spl_token::instruction::initialize_account(&self.program_id(), account, mint, owner)
                    .unwrap()
            }
            Self::Token2022 => spl_token_2022::instruction::initialize_account(
                &self.program_id(),
                account,
                mint,
                owner,
            )
            .unwrap(),
        }
    }

    fn mint_to(
        self,
        mint: &Pubkey,
        account: &Pubkey,
        authority: &Pubkey,
        amount: u64,
    ) -> Instruction {
        match self {
            Self::Spl => spl_token::instruction::mint_to(
                &self.program_id(),
                mint,
                account,
                authority,
                &[],
                amount,
            )
            .unwrap(),
            Self::Token2022 => spl_token_2022::instruction::mint_to(
                &self.program_id(),
                mint,
                account,
                authority,
                &[],
                amount,
            )
            .unwrap(),
        }
    }

    fn revoke_mint_authority(self, mint: &Pubkey, authority: &Pubkey) -> Instruction {
        match self {
            Self::Spl => spl_token::instruction::set_authority(
                &self.program_id(),
                mint,
                None,
                spl_token::instruction::AuthorityType::MintTokens,
                authority,
                &[],
            )
            .unwrap(),
            Self::Token2022 => spl_token_2022::instruction::set_authority(
                &self.program_id(),
                mint,
                None,
                spl_token_2022::instruction::AuthorityType::MintTokens,
                authority,
                &[],
            )
            .unwrap(),
        }
    }

    fn set_account_owner(
        self,
        account: &Pubkey,
        new_owner: &Pubkey,
        current_owner: &Pubkey,
    ) -> Instruction {
        match self {
            Self::Spl => spl_token::instruction::set_authority(
                &self.program_id(),
                account,
                Some(new_owner),
                spl_token::instruction::AuthorityType::AccountOwner,
                current_owner,
                &[],
            )
            .unwrap(),
            Self::Token2022 => spl_token_2022::instruction::set_authority(
                &self.program_id(),
                account,
                Some(new_owner),
                spl_token_2022::instruction::AuthorityType::AccountOwner,
                current_owner,
                &[],
            )
            .unwrap(),
        }
    }

    fn set_close_authority(
        self,
        account: &Pubkey,
        close_authority: &Pubkey,
        current_owner: &Pubkey,
    ) -> Instruction {
        match self {
            Self::Spl => spl_token::instruction::set_authority(
                &self.program_id(),
                account,
                Some(close_authority),
                spl_token::instruction::AuthorityType::CloseAccount,
                current_owner,
                &[],
            )
            .unwrap(),
            Self::Token2022 => spl_token_2022::instruction::set_authority(
                &self.program_id(),
                account,
                Some(close_authority),
                spl_token_2022::instruction::AuthorityType::CloseAccount,
                current_owner,
                &[],
            )
            .unwrap(),
        }
    }
}

async fn start() -> ProgramTestContext {
    let mut program_test = ProgramTest::new("arena_lock_v2", id(), processor!(process_instruction));
    program_test.add_program(
        "spl_token",
        spl_token::id(),
        processor!(spl_token::processor::Processor::process),
    );
    program_test.add_program(
        "spl_token_2022",
        spl_token_2022::id(),
        processor!(spl_token_2022::processor::Processor::process),
    );
    program_test.start_with_context().await
}

async fn process_tx(
    context: &mut ProgramTestContext,
    signers: &[&Keypair],
    instructions: Vec<Instruction>,
) {
    let recent_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let mut signer_refs = vec![&context.payer];
    for signer in signers {
        if signer.pubkey() != context.payer.pubkey() {
            signer_refs.push(*signer);
        }
    }
    let tx = Transaction::new_signed_with_payer(
        &instructions,
        Some(&context.payer.pubkey()),
        &signer_refs,
        recent_blockhash,
    );
    context.banks_client.process_transaction(tx).await.unwrap();
}

async fn process_tx_expect_err(
    context: &mut ProgramTestContext,
    signers: &[&Keypair],
    instructions: Vec<Instruction>,
) {
    let recent_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let mut signer_refs = vec![&context.payer];
    for signer in signers {
        if signer.pubkey() != context.payer.pubkey() {
            signer_refs.push(*signer);
        }
    }
    let tx = Transaction::new_signed_with_payer(
        &instructions,
        Some(&context.payer.pubkey()),
        &signer_refs,
        recent_blockhash,
    );
    assert!(context.banks_client.process_transaction(tx).await.is_err());
}

async fn advance_time(context: &mut ProgramTestContext, seconds: i64) {
    let mut clock = context.banks_client.get_sysvar::<Clock>().await.unwrap();
    context.warp_to_slot(clock.slot + 1).unwrap();
    clock = context.banks_client.get_sysvar::<Clock>().await.unwrap();
    clock.unix_timestamp += seconds;
    context.set_sysvar(&clock);
}

/// H-02 helper: after activate, stake is warming. Advance one epoch roll and
/// touch the position so warming matures into eligible.
async fn mature_after_activate(fixture: &mut ArenaFixture, flavor: TokenFlavor) {
    let epoch_pad = {
        let config = load_config(&mut fixture.context, fixture.config).await;
        config.epoch_seconds + 1
    };
    advance_time(&mut fixture.context, epoch_pad).await;
    process_tx(
        &mut fixture.context,
        &[],
        vec![roll_epoch(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    // Sync path: activate with no pending promotes warming after epoch advance.
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    let position = load_position(&mut fixture.context, fixture.position).await;
    assert_eq!(position.warming_amount, 0);
    assert!(position.eligible_amount > 0);
}

async fn fund(context: &mut ProgramTestContext, to: &Pubkey, lamports: u64) {
    process_tx(
        context,
        &[],
        vec![system_instruction::transfer(
            &context.payer.pubkey(),
            to,
            lamports,
        )],
    )
    .await;
}

async fn create_mint_and_account(
    context: &mut ProgramTestContext,
    flavor: TokenFlavor,
    mint: &Keypair,
    owner: &Pubkey,
    account: &Keypair,
    decimals: u8,
) {
    let rent = context.banks_client.get_rent().await.unwrap();
    let token_program = flavor.program_id();
    process_tx(
        context,
        &[mint, account],
        vec![
            system_instruction::create_account(
                &context.payer.pubkey(),
                &mint.pubkey(),
                rent.minimum_balance(flavor.mint_len()),
                flavor.mint_len() as u64,
                &token_program,
            ),
            flavor.initialize_mint(&mint.pubkey(), &context.payer.pubkey(), decimals),
            system_instruction::create_account(
                &context.payer.pubkey(),
                &account.pubkey(),
                rent.minimum_balance(flavor.account_len()),
                flavor.account_len() as u64,
                &token_program,
            ),
            flavor.initialize_account(&account.pubkey(), &mint.pubkey(), owner),
        ],
    )
    .await;
}

async fn create_token_account(
    context: &mut ProgramTestContext,
    flavor: TokenFlavor,
    mint: &Pubkey,
    owner: &Pubkey,
    account: &Keypair,
) {
    let rent = context.banks_client.get_rent().await.unwrap();
    let token_program = flavor.program_id();
    process_tx(
        context,
        &[account],
        vec![
            system_instruction::create_account(
                &context.payer.pubkey(),
                &account.pubkey(),
                rent.minimum_balance(flavor.account_len()),
                flavor.account_len() as u64,
                &token_program,
            ),
            flavor.initialize_account(&account.pubkey(), mint, owner),
        ],
    )
    .await;
}

async fn mint_to_account(
    context: &mut ProgramTestContext,
    flavor: TokenFlavor,
    mint: &Pubkey,
    account: &Pubkey,
    amount: u64,
) {
    process_tx(
        context,
        &[],
        vec![flavor.mint_to(mint, account, &context.payer.pubkey(), amount)],
    )
    .await;
}

async fn revoke_mint_authority(
    context: &mut ProgramTestContext,
    flavor: TokenFlavor,
    mint: &Pubkey,
) {
    let authority = context.payer.pubkey();
    process_tx(
        context,
        &[],
        vec![flavor.revoke_mint_authority(mint, &authority)],
    )
    .await;
}

async fn token_balance(
    context: &mut ProgramTestContext,
    flavor: TokenFlavor,
    account: Pubkey,
) -> u64 {
    let account = context
        .banks_client
        .get_account(account)
        .await
        .unwrap()
        .unwrap();
    match flavor {
        TokenFlavor::Spl => {
            spl_token::state::Account::unpack(&account.data)
                .unwrap()
                .amount
        }
        TokenFlavor::Token2022 => {
            spl_token_2022::state::Account::unpack(&account.data)
                .unwrap()
                .amount
        }
    }
}

async fn load_config(context: &mut ProgramTestContext, config: Pubkey) -> ArenaConfig {
    let account = context
        .banks_client
        .get_account(config)
        .await
        .unwrap()
        .unwrap();
    ArenaConfig::load(&account.data).unwrap()
}

async fn load_position(context: &mut ProgramTestContext, position: Pubkey) -> ArenaPosition {
    let account = context
        .banks_client
        .get_account(position)
        .await
        .unwrap()
        .unwrap();
    ArenaPosition::load(&account.data).unwrap()
}

fn program_account(data: Vec<u8>, lamports: u64) -> AccountSharedData {
    AccountSharedData::from(Account {
        lamports,
        data,
        owner: id(),
        executable: false,
        rent_epoch: 0,
    })
}

struct ArenaFixture {
    context: ProgramTestContext,
    authority: Keypair,
    user: Keypair,
    funder: Keypair,
    mint: Keypair,
    user_token: Keypair,
    funder_token: Keypair,
    vault_token: Keypair,
    reward_pool_token: Keypair,
    config_id: u64,
    config: Pubkey,
    position: Pubkey,
}

async fn setup_arena(
    flavor: TokenFlavor,
    config_id: u64,
    min_lock_seconds: i64,
    activation_delay_seconds: i64,
    epoch_seconds: i64,
    penalty_bps: u16,
    burn_bps: u16,
) -> ArenaFixture {
    setup_arena_with_min_deposit(
        flavor,
        config_id,
        min_lock_seconds,
        activation_delay_seconds,
        epoch_seconds,
        penalty_bps,
        burn_bps,
        100,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn setup_arena_with_min_deposit(
    flavor: TokenFlavor,
    config_id: u64,
    min_lock_seconds: i64,
    activation_delay_seconds: i64,
    epoch_seconds: i64,
    penalty_bps: u16,
    burn_bps: u16,
    min_deposit_amount: u64,
) -> ArenaFixture {
    let mut context = start().await;
    let authority = Keypair::new();
    let user = Keypair::new();
    let funder = Keypair::new();
    let mint = Keypair::new();
    let user_token = Keypair::new();
    let funder_token = Keypair::new();
    let vault_token = Keypair::new();
    let reward_pool_token = Keypair::new();

    fund(&mut context, &authority.pubkey(), LAMPORTS_PER_SOL).await;
    fund(&mut context, &user.pubkey(), LAMPORTS_PER_SOL).await;
    fund(&mut context, &funder.pubkey(), LAMPORTS_PER_SOL).await;

    let (config, _) = config_pda(&id(), &authority.pubkey(), config_id);
    let (position, _) = position_pda(&id(), &config, &user.pubkey());
    let (vault_authority, _) = vault_authority_pda(&id(), &config);

    create_mint_and_account(
        &mut context,
        flavor,
        &mint,
        &user.pubkey(),
        &user_token,
        DECIMALS,
    )
    .await;
    create_token_account(
        &mut context,
        flavor,
        &mint.pubkey(),
        &funder.pubkey(),
        &funder_token,
    )
    .await;
    create_token_account(
        &mut context,
        flavor,
        &mint.pubkey(),
        &vault_authority,
        &vault_token,
    )
    .await;
    create_token_account(
        &mut context,
        flavor,
        &mint.pubkey(),
        &vault_authority,
        &reward_pool_token,
    )
    .await;
    mint_to_account(
        &mut context,
        flavor,
        &mint.pubkey(),
        &user_token.pubkey(),
        USER_STARTING_TOKENS,
    )
    .await;
    mint_to_account(
        &mut context,
        flavor,
        &mint.pubkey(),
        &funder_token.pubkey(),
        USER_STARTING_TOKENS,
    )
    .await;
    revoke_mint_authority(&mut context, flavor, &mint.pubkey()).await;

    process_tx(
        &mut context,
        &[&authority],
        vec![initialize_config(
            id(),
            authority.pubkey(),
            config_id,
            min_lock_seconds,
            activation_delay_seconds,
            epoch_seconds,
            min_deposit_amount,
            penalty_bps,
            burn_bps,
            mint.pubkey(),
            vault_token.pubkey(),
            reward_pool_token.pubkey(),
            flavor.program_id(),
            DECIMALS,
        )],
    )
    .await;

    ArenaFixture {
        context,
        authority,
        user,
        funder,
        mint,
        user_token,
        funder_token,
        vault_token,
        reward_pool_token,
        config_id,
        config,
        position,
    }
}

#[tokio::test]
async fn rejects_program_owned_config_at_wrong_pda() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena(flavor, 6, 1_000, 10, 10, 1_000, 100).await;
    let fake_config_key = Pubkey::new_unique();
    let rent = fixture.context.banks_client.get_rent().await.unwrap();
    let mut fake_config = load_config(&mut fixture.context, fixture.config).await;
    fake_config.last_epoch_ts = 0;
    fake_config.epoch_seconds = 1;
    let mut fake_config_data = vec![0; CONFIG_SIZE];
    fake_config.store(&mut fake_config_data).unwrap();
    let fake_config_account = program_account(fake_config_data, rent.minimum_balance(CONFIG_SIZE));
    fixture
        .context
        .set_account(&fake_config_key, &fake_config_account);

    let mut ix = roll_epoch(
        id(),
        fixture.authority.pubkey(),
        fixture.config_id,
        fixture.reward_pool_token.pubkey(),
        fixture.mint.pubkey(),
        flavor.program_id(),
    );
    ix.accounts[0].pubkey = fake_config_key;
    process_tx_expect_err(&mut fixture.context, &[], vec![ix]).await;
}

#[tokio::test]
async fn rejects_program_owned_position_at_wrong_pda() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena(flavor, 7, 1_000, 10, 10, 1_000, 100).await;
    let amount = 1_000;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            amount,
        )],
    )
    .await;

    let fake_position_key = Pubkey::new_unique();
    let rent = fixture.context.banks_client.get_rent().await.unwrap();
    let fake_position = ArenaPosition {
        is_initialized: true,
        version: ARENA_POSITION_VERSION,
        config: fixture.config,
        owner: fixture.user.pubkey(),
        locked_amount: amount,
        eligible_amount: 0,
        pending_activation_amount: amount,
        total_deposited: amount,
        total_withdrawn: 0,
        total_penalty_paid: 0,
        total_burned: 0,
        total_rewards_claimed: 0,
        pending_rewards: 0,
        lock_start_ts: 0,
        unlock_ts: 1_000,
        activation_ts: 0,
        last_activity_ts: 0,
        reward_index_checkpoint: 0,
        reward_index_generation_checkpoint: 0,
        position_bump: 0,
        warming_amount: 0,
        warming_epoch: 0,
        penalty_remainder: 0,
        burn_remainder: 0,
        reward_accrual_remainder: 0,
    };
    let mut fake_position_data = vec![0; POSITION_SIZE];
    fake_position.store(&mut fake_position_data).unwrap();
    let fake_position_account =
        program_account(fake_position_data, rent.minimum_balance(POSITION_SIZE));
    fixture
        .context
        .set_account(&fake_position_key, &fake_position_account);

    let mut ix = activate_position(
        id(),
        fixture.authority.pubkey(),
        fixture.config_id,
        fixture.user.pubkey(),
        fixture.reward_pool_token.pubkey(),
        fixture.mint.pubkey(),
        flavor.program_id(),
    );
    ix.accounts[2].pubkey = fake_position_key;
    process_tx_expect_err(&mut fixture.context, &[&fixture.user], vec![ix]).await;
}

#[tokio::test]
async fn rejects_attacker_position_and_destination_substitution() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena(flavor, 8, 1_000, 10, 10, 1_000, 100).await;
    let attacker = Keypair::new();
    let attacker_token = Keypair::new();
    let amount = 1_000;

    fund(&mut fixture.context, &attacker.pubkey(), LAMPORTS_PER_SOL).await;
    create_token_account(
        &mut fixture.context,
        flavor,
        &fixture.mint.pubkey(),
        &attacker.pubkey(),
        &attacker_token,
    )
    .await;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            amount,
        )],
    )
    .await;

    let mut attacker_withdraw = withdraw(
        id(),
        fixture.authority.pubkey(),
        fixture.config_id,
        attacker.pubkey(),
        attacker_token.pubkey(),
        fixture.vault_token.pubkey(),
        fixture.reward_pool_token.pubkey(),
        fixture.mint.pubkey(),
        flavor.program_id(),
        100,
    );
    attacker_withdraw.accounts[3].pubkey = fixture.position;
    process_tx_expect_err(&mut fixture.context, &[&attacker], vec![attacker_withdraw]).await;

    let redirected_withdraw = withdraw(
        id(),
        fixture.authority.pubkey(),
        fixture.config_id,
        fixture.user.pubkey(),
        attacker_token.pubkey(),
        fixture.vault_token.pubkey(),
        fixture.reward_pool_token.pubkey(),
        fixture.mint.pubkey(),
        flavor.program_id(),
        100,
    );
    process_tx_expect_err(
        &mut fixture.context,
        &[&fixture.user],
        vec![redirected_withdraw],
    )
    .await;
}

#[tokio::test]
async fn rejects_reward_funding_without_eligible_stake() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena(flavor, 11, 1_000, 10, 10, 1_000, 100).await;

    process_tx_expect_err(
        &mut fixture.context,
        &[&fixture.funder],
        vec![fund_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.funder.pubkey(),
            fixture.funder_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            100_000,
        )],
    )
    .await;
}

#[tokio::test]
async fn checked_reward_funding_rejects_stale_eligible_snapshot() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena_with_min_deposit(flavor, 97, 1_000, 5, 10, 0, 0, 1).await;
    let deposit_amount = 100_000;
    let reward_amount = 25_000;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            deposit_amount,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 6).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    mature_after_activate(&mut fixture, flavor).await;

    let config = load_config(&mut fixture.context, fixture.config).await;
    assert_eq!(config.eligible_locked, deposit_amount);
    let funder_before =
        token_balance(&mut fixture.context, flavor, fixture.funder_token.pubkey()).await;
    let pool_before = token_balance(
        &mut fixture.context,
        flavor,
        fixture.reward_pool_token.pubkey(),
    )
    .await;

    process_tx_expect_err(
        &mut fixture.context,
        &[&fixture.funder],
        vec![fund_rewards_checked(
            id(),
            fixture.funder.pubkey(),
            fixture.config_id,
            fixture.authority.pubkey(),
            fixture.funder_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            reward_amount,
            deposit_amount - 1,
            config.current_epoch,
        )],
    )
    .await;

    assert_eq!(
        token_balance(&mut fixture.context, flavor, fixture.funder_token.pubkey()).await,
        funder_before
    );
    assert_eq!(
        token_balance(
            &mut fixture.context,
            flavor,
            fixture.reward_pool_token.pubkey()
        )
        .await,
        pool_before
    );

    process_tx(
        &mut fixture.context,
        &[&fixture.funder],
        vec![fund_rewards_checked(
            id(),
            fixture.funder.pubkey(),
            fixture.config_id,
            fixture.authority.pubkey(),
            fixture.funder_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            reward_amount,
            deposit_amount,
            config.current_epoch,
        )],
    )
    .await;
    let config = load_config(&mut fixture.context, fixture.config).await;
    assert_eq!(config.total_rewards_funded, reward_amount);
    assert_eq!(config.total_rewards_distributed, reward_amount);
}

#[tokio::test]
async fn rejects_claim_from_zero_stake_orphan_position() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena(flavor, 12, 1_000, 10, 10, 1_000, 100).await;
    let (_, position_bump) = position_pda(&id(), &fixture.config, &fixture.user.pubkey());
    let rent = fixture.context.banks_client.get_rent().await.unwrap();
    let orphan_position = ArenaPosition {
        is_initialized: true,
        version: ARENA_POSITION_VERSION,
        config: fixture.config,
        owner: fixture.user.pubkey(),
        locked_amount: 0,
        eligible_amount: 0,
        pending_activation_amount: 0,
        total_deposited: 1_000,
        total_withdrawn: 1_000,
        total_penalty_paid: 0,
        total_burned: 0,
        total_rewards_claimed: 0,
        pending_rewards: 1_000,
        lock_start_ts: 0,
        unlock_ts: 0,
        activation_ts: 0,
        last_activity_ts: 0,
        reward_index_checkpoint: 0,
        reward_index_generation_checkpoint: 0,
        position_bump,
        warming_amount: 0,
        warming_epoch: 0,
        penalty_remainder: 0,
        burn_remainder: 0,
        reward_accrual_remainder: 0,
    };
    let mut position_data = vec![0; POSITION_SIZE];
    orphan_position.store(&mut position_data).unwrap();
    let position_account = program_account(position_data, rent.minimum_balance(POSITION_SIZE));
    fixture
        .context
        .set_account(&fixture.position, &position_account);

    process_tx_expect_err(
        &mut fixture.context,
        &[&fixture.user],
        vec![claim_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
}

#[tokio::test]
async fn activation_epoch_roll_and_claim_rewards() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena(flavor, 1, 1_000, 10, 10, 1_000, 100).await;
    let amount = 1_000_000;
    let reward = 100_000;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            amount,
        )],
    )
    .await;

    process_tx_expect_err(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;

    advance_time(&mut fixture.context, 11).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;

    let position = load_position(&mut fixture.context, fixture.position).await;
    assert_eq!(position.warming_amount, amount);
    assert_eq!(position.eligible_amount, 0);
    assert_eq!(position.pending_activation_amount, 0);

    // H-02: must mature across an epoch boundary before funding/rewards.
    mature_after_activate(&mut fixture, flavor).await;
    let position = load_position(&mut fixture.context, fixture.position).await;
    assert_eq!(position.eligible_amount, amount);

    process_tx(
        &mut fixture.context,
        &[&fixture.funder],
        vec![fund_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.funder.pubkey(),
            fixture.funder_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            reward,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 11).await;
    process_tx(
        &mut fixture.context,
        &[],
        vec![roll_epoch(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![claim_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;

    let config = load_config(&mut fixture.context, fixture.config).await;
    assert_eq!(config.total_rewards_funded, reward);
    assert_eq!(config.total_rewards_distributed, reward);
    assert_eq!(config.total_rewards_claimed, reward);
    assert_eq!(
        token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await,
        USER_STARTING_TOKENS - amount + reward
    );
    assert_eq!(
        token_balance(
            &mut fixture.context,
            flavor,
            fixture.reward_pool_token.pubkey()
        )
        .await,
        0
    );
}

#[tokio::test]
async fn early_exit_splits_penalty_between_rewards_and_burn() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena(flavor, 2, 1_000, 10, 10, 1_000, 100).await;
    let amount = 1_000_000;
    let withdraw_amount = 100_000;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            amount,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 11).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![withdraw(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            withdraw_amount,
        )],
    )
    .await;

    let config = load_config(&mut fixture.context, fixture.config).await;
    let position = load_position(&mut fixture.context, fixture.position).await;
    assert_eq!(config.total_locked, 900_000);
    // Still warming (not yet matured across an epoch) — stake lives in warming_locked.
    assert_eq!(config.eligible_locked, 0);
    assert_eq!(config.warming_locked, 900_000);
    assert_eq!(config.pending_activation_locked, 0);
    assert_eq!(config.pending_rewards, 0);
    assert_eq!(config.total_penalties_collected, 10_000);
    assert_eq!(config.total_burned, 10_000);
    assert_eq!(position.total_penalty_paid, 10_000);
    assert_eq!(position.total_burned, 10_000);
    assert_eq!(
        token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await,
        USER_STARTING_TOKENS - amount + 90_000
    );
    assert_eq!(
        token_balance(
            &mut fixture.context,
            flavor,
            fixture.reward_pool_token.pubkey()
        )
        .await,
        0
    );
    assert_eq!(
        token_balance(&mut fixture.context, flavor, fixture.vault_token.pubkey()).await,
        900_000
    );
}

#[tokio::test]
async fn early_exit_with_no_remaining_eligible_burns_full_penalty() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena(flavor, 10, 1_000, 10, 10, 1_000, 100).await;
    let amount = 1_000_000;
    let reward = 100_000;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            amount,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 11).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    mature_after_activate(&mut fixture, flavor).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.funder],
        vec![fund_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.funder.pubkey(),
            fixture.funder_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            reward,
        )],
    )
    .await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![withdraw(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            amount,
        )],
    )
    .await;

    let config = load_config(&mut fixture.context, fixture.config).await;
    let position = load_position(&mut fixture.context, fixture.position).await;
    assert_eq!(config.total_locked, 0);
    assert_eq!(config.eligible_locked, 0);
    assert_eq!(config.pending_rewards, 0);
    assert_eq!(config.total_rewards_expired, 0);
    assert_eq!(config.total_penalties_collected, 100_000);
    assert_eq!(config.total_burned, 100_000);
    assert_eq!(position.total_penalty_paid, 100_000);
    assert_eq!(position.total_burned, 100_000);
    assert_eq!(
        token_balance(
            &mut fixture.context,
            flavor,
            fixture.reward_pool_token.pubkey()
        )
        .await,
        0
    );
    assert_eq!(
        token_balance(&mut fixture.context, flavor, fixture.vault_token.pubkey()).await,
        0
    );
}

#[tokio::test]
async fn full_exit_settles_rewards_and_leaves_no_post_exit_claim() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena(flavor, 5, 1_000, 10, 10, 1_000, 100).await;
    let amount = 1_000_000;
    let reward = 100_000;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            amount,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 11).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    mature_after_activate(&mut fixture, flavor).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.funder],
        vec![fund_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.funder.pubkey(),
            fixture.funder_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            reward,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 11).await;
    process_tx(
        &mut fixture.context,
        &[],
        vec![roll_epoch(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![withdraw(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            amount,
        )],
    )
    .await;

    let config = load_config(&mut fixture.context, fixture.config).await;
    let position = load_position(&mut fixture.context, fixture.position).await;
    assert_eq!(config.total_locked, 0);
    assert_eq!(config.eligible_locked, 0);
    assert_eq!(config.pending_rewards, 0);
    assert_eq!(config.total_rewards_distributed, reward);
    assert_eq!(config.total_rewards_claimed, reward);
    assert_eq!(config.total_penalties_collected, 100_000);
    assert_eq!(config.total_burned, 100_000);
    assert_eq!(position.locked_amount, 0);
    assert_eq!(position.eligible_amount, 0);
    assert_eq!(position.pending_rewards, 0);
    assert_eq!(position.total_rewards_claimed, reward);
    assert_eq!(position.total_withdrawn, 900_000);
    assert_eq!(position.total_penalty_paid, 100_000);
    assert_eq!(position.total_burned, 100_000);
    assert_eq!(
        token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await,
        USER_STARTING_TOKENS
    );
    assert_eq!(
        token_balance(
            &mut fixture.context,
            flavor,
            fixture.reward_pool_token.pubkey()
        )
        .await,
        0
    );
    assert_eq!(
        token_balance(&mut fixture.context, flavor, fixture.vault_token.pubkey()).await,
        0
    );

    process_tx_expect_err(
        &mut fixture.context,
        &[&fixture.user],
        vec![claim_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
}

#[tokio::test]
async fn token_2022_plain_accounts_support_burn_and_reward_pool_path() {
    let flavor = TokenFlavor::Token2022;
    let mut fixture = setup_arena(flavor, 3, 1_000, 10, 10, 1_000, 100).await;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            1_000,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 11).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![withdraw(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            500,
        )],
    )
    .await;

    let config = load_config(&mut fixture.context, fixture.config).await;
    assert_eq!(config.total_burned, 50);
    assert_eq!(config.pending_rewards, 0);
}

#[tokio::test]
async fn rejects_burn_share_above_total_penalty() {
    let flavor = TokenFlavor::Spl;
    let mut context = start().await;
    let authority = Keypair::new();
    let user = Keypair::new();
    let mint = Keypair::new();
    let user_token = Keypair::new();
    let vault_token = Keypair::new();
    let reward_pool_token = Keypair::new();
    let config_id = 4;

    fund(&mut context, &authority.pubkey(), LAMPORTS_PER_SOL).await;
    let (config, _) = config_pda(&id(), &authority.pubkey(), config_id);
    let (vault_authority, _) = vault_authority_pda(&id(), &config);
    create_mint_and_account(
        &mut context,
        flavor,
        &mint,
        &user.pubkey(),
        &user_token,
        DECIMALS,
    )
    .await;
    create_token_account(
        &mut context,
        flavor,
        &mint.pubkey(),
        &vault_authority,
        &vault_token,
    )
    .await;
    create_token_account(
        &mut context,
        flavor,
        &mint.pubkey(),
        &vault_authority,
        &reward_pool_token,
    )
    .await;

    process_tx_expect_err(
        &mut context,
        &[&authority],
        vec![initialize_config(
            id(),
            authority.pubkey(),
            config_id,
            1_000,
            10,
            10,
            100,
            1_000,
            1_001,
            mint.pubkey(),
            vault_token.pubkey(),
            reward_pool_token.pubkey(),
            flavor.program_id(),
            DECIMALS,
        )],
    )
    .await;
}

#[tokio::test]
async fn rejects_mint_with_mint_authority() {
    let flavor = TokenFlavor::Spl;
    let mut context = start().await;
    let authority = Keypair::new();
    let user = Keypair::new();
    let mint = Keypair::new();
    let user_token = Keypair::new();
    let vault_token = Keypair::new();
    let reward_pool_token = Keypair::new();
    let config_id = 8;

    fund(&mut context, &authority.pubkey(), LAMPORTS_PER_SOL).await;
    let (config, _) = config_pda(&id(), &authority.pubkey(), config_id);
    let (vault_authority, _) = vault_authority_pda(&id(), &config);
    create_mint_and_account(
        &mut context,
        flavor,
        &mint,
        &user.pubkey(),
        &user_token,
        DECIMALS,
    )
    .await;
    create_token_account(
        &mut context,
        flavor,
        &mint.pubkey(),
        &vault_authority,
        &vault_token,
    )
    .await;
    create_token_account(
        &mut context,
        flavor,
        &mint.pubkey(),
        &vault_authority,
        &reward_pool_token,
    )
    .await;

    process_tx_expect_err(
        &mut context,
        &[&authority],
        vec![initialize_config(
            id(),
            authority.pubkey(),
            config_id,
            1_000,
            10,
            10,
            100,
            1_000,
            100,
            mint.pubkey(),
            vault_token.pubkey(),
            reward_pool_token.pubkey(),
            flavor.program_id(),
            DECIMALS,
        )],
    )
    .await;
}

#[tokio::test]
async fn rejects_mint_with_freeze_authority() {
    let flavor = TokenFlavor::Spl;
    let mut context = start().await;
    let authority = Keypair::new();
    let freeze_authority = Keypair::new();
    let mint = Keypair::new();
    let vault_token = Keypair::new();
    let reward_pool_token = Keypair::new();
    let config_id = 9;

    fund(&mut context, &authority.pubkey(), LAMPORTS_PER_SOL).await;
    let rent = context.banks_client.get_rent().await.unwrap();
    let payer = context.payer.pubkey();
    let (config, _) = config_pda(&id(), &authority.pubkey(), config_id);
    let (vault_authority, _) = vault_authority_pda(&id(), &config);

    process_tx(
        &mut context,
        &[&mint],
        vec![
            system_instruction::create_account(
                &payer,
                &mint.pubkey(),
                rent.minimum_balance(flavor.mint_len()),
                flavor.mint_len() as u64,
                &flavor.program_id(),
            ),
            spl_token::instruction::initialize_mint(
                &flavor.program_id(),
                &mint.pubkey(),
                &payer,
                Some(&freeze_authority.pubkey()),
                DECIMALS,
            )
            .unwrap(),
        ],
    )
    .await;
    create_token_account(
        &mut context,
        flavor,
        &mint.pubkey(),
        &vault_authority,
        &vault_token,
    )
    .await;
    create_token_account(
        &mut context,
        flavor,
        &mint.pubkey(),
        &vault_authority,
        &reward_pool_token,
    )
    .await;

    process_tx_expect_err(
        &mut context,
        &[&authority],
        vec![initialize_config(
            id(),
            authority.pubkey(),
            config_id,
            1_000,
            10,
            10,
            100,
            1_000,
            100,
            mint.pubkey(),
            vault_token.pubkey(),
            reward_pool_token.pubkey(),
            flavor.program_id(),
            DECIMALS,
        )],
    )
    .await;
}

#[tokio::test]
async fn rejects_custody_token_account_with_close_authority() {
    let flavor = TokenFlavor::Spl;
    let mut context = start().await;
    let authority = Keypair::new();
    let user = Keypair::new();
    let temp_owner = Keypair::new();
    let close_authority = Keypair::new();
    let mint = Keypair::new();
    let user_token = Keypair::new();
    let vault_token = Keypair::new();
    let reward_pool_token = Keypair::new();
    let config_id = 10;

    fund(&mut context, &authority.pubkey(), LAMPORTS_PER_SOL).await;
    let (config, _) = config_pda(&id(), &authority.pubkey(), config_id);
    let (vault_authority, _) = vault_authority_pda(&id(), &config);
    create_mint_and_account(
        &mut context,
        flavor,
        &mint,
        &user.pubkey(),
        &user_token,
        DECIMALS,
    )
    .await;
    create_token_account(
        &mut context,
        flavor,
        &mint.pubkey(),
        &temp_owner.pubkey(),
        &vault_token,
    )
    .await;
    create_token_account(
        &mut context,
        flavor,
        &mint.pubkey(),
        &vault_authority,
        &reward_pool_token,
    )
    .await;
    process_tx(
        &mut context,
        &[&temp_owner],
        vec![
            flavor.set_close_authority(
                &vault_token.pubkey(),
                &close_authority.pubkey(),
                &temp_owner.pubkey(),
            ),
            flavor.set_account_owner(
                &vault_token.pubkey(),
                &vault_authority,
                &temp_owner.pubkey(),
            ),
        ],
    )
    .await;
    revoke_mint_authority(&mut context, flavor, &mint.pubkey()).await;

    process_tx_expect_err(
        &mut context,
        &[&authority],
        vec![initialize_config(
            id(),
            authority.pubkey(),
            config_id,
            1_000,
            10,
            10,
            100,
            1_000,
            100,
            mint.pubkey(),
            vault_token.pubkey(),
            reward_pool_token.pubkey(),
            flavor.program_id(),
            DECIMALS,
        )],
    )
    .await;
}

#[tokio::test]
async fn deposit_extends_unlock_without_shortening_existing_lock() {
    let flavor = TokenFlavor::Spl;
    // min_lock=100s, activation=10s
    let mut fixture = setup_arena(flavor, 20, 1_000, 100, 10, 1_000, 100).await;
    let first = 1_000_000u64;
    let second = 100_000u64;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            first,
        )],
    )
    .await;

    let after_first = load_position(&mut fixture.context, fixture.position).await;
    let first_unlock = after_first.unlock_ts;
    assert!(first_unlock > 0);

    // Near the end of the lock window, top up with a shorter residual would
    // previously reset unlock to now+min_lock and could shorten relative to
    // a longer remaining lock. Advance only 10s so remaining lock is still long.
    advance_time(&mut fixture.context, 10).await;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            second,
        )],
    )
    .await;

    let after_second = load_position(&mut fixture.context, fixture.position).await;
    assert_eq!(after_second.lock_start_ts, after_first.lock_start_ts);
    assert!(
        after_second.unlock_ts >= first_unlock,
        "top-up must not shorten unlock_ts (was {first_unlock}, now {})",
        after_second.unlock_ts
    );
    assert_eq!(after_second.locked_amount, first + second);
    assert_eq!(after_second.pending_activation_amount, first + second);
}

#[tokio::test]
async fn tiny_early_exit_with_floor_penalty_returns_principal() {
    let flavor = TokenFlavor::Spl;
    // min_deposit in setup is 100; use 50% early exit bps so floor(1 * 5000/10000)=0
    let mut fixture = setup_arena(flavor, 21, 1_000, 10, 10, 5_000, 1_000).await;
    let deposit_amount = 100u64;
    let withdraw_amount = 1u64;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            deposit_amount,
        )],
    )
    .await;

    // Early exit 1 unit before unlock — floor penalty is 0, full unit returned
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![withdraw(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            withdraw_amount,
        )],
    )
    .await;

    let position = load_position(&mut fixture.context, fixture.position).await;
    assert_eq!(position.locked_amount, deposit_amount - withdraw_amount);
    assert_eq!(position.total_penalty_paid, 0);
    assert_eq!(
        token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await,
        USER_STARTING_TOKENS - deposit_amount + withdraw_amount
    );
}

/// H-01 regression: floor remainder must not be re-indexed on later rolls.
///
/// Pre-fix: eligible=3, fund=2 → after three rolls a sole position could claim 3
/// (phantom 1) and then steal newly funded tokens. Post-fix claim ≤ funded.
#[tokio::test]
async fn roll_epoch_does_not_recredit_reward_remainder() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena_with_min_deposit(flavor, 90, 1_000, 5, 10, 0, 0, 1).await;
    let stake = 3u64;
    let funded = 2u64;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            stake,
        )],
    )
    .await;

    advance_time(&mut fixture.context, 6).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    mature_after_activate(&mut fixture, flavor).await;

    process_tx(
        &mut fixture.context,
        &[&fixture.funder],
        vec![fund_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.funder.pubkey(),
            fixture.funder_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            funded,
        )],
    )
    .await;

    // Three epoch rolls — pre-fix this minted phantom index credit each time.
    for _ in 0..3 {
        advance_time(&mut fixture.context, 11).await;
        process_tx(
            &mut fixture.context,
            &[],
            vec![roll_epoch(
                id(),
                fixture.authority.pubkey(),
                fixture.config_id,
                fixture.reward_pool_token.pubkey(),
                fixture.mint.pubkey(),
                flavor.program_id(),
            )],
        )
        .await;
    }

    let config_after_rolls = load_config(&mut fixture.context, fixture.config).await;
    assert_eq!(
        config_after_rolls.pending_rewards, 0,
        "funded batch is indexed immediately"
    );
    assert_eq!(config_after_rolls.total_rewards_distributed, funded);
    assert_eq!(config_after_rolls.reward_dust, 0);

    let balance_before_claim =
        token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![claim_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;

    let claimed = token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await
        - balance_before_claim;
    assert!(
        claimed <= funded,
        "H-01: claimed {claimed} exceeds funded {funded} after multi-roll remainder re-credit"
    );
    assert_eq!(claimed, 1, "sole staker earns floor(2*SCALE/3)*3/SCALE = 1");

    // Fund +1 more and roll once: total lifetime claim must stay ≤ total funded.
    let extra = 1u64;
    process_tx(
        &mut fixture.context,
        &[&fixture.funder],
        vec![fund_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.funder.pubkey(),
            fixture.funder_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            extra,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 11).await;
    process_tx(
        &mut fixture.context,
        &[],
        vec![roll_epoch(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;

    let config_after_extra = load_config(&mut fixture.context, fixture.config).await;
    assert_eq!(config_after_extra.pending_rewards, 0);
    assert_eq!(config_after_extra.total_rewards_distributed, funded + extra);
    let before_second_claim =
        token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![claim_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    let second_claimed = token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey())
        .await
        - before_second_claim;
    assert_eq!(
        second_claimed, 1,
        "fractional carry can crystallize later funding"
    );

    let config = load_config(&mut fixture.context, fixture.config).await;
    assert_eq!(config.total_rewards_claimed, claimed + second_claimed);
    assert!(
        config.total_rewards_claimed <= config.total_rewards_funded,
        "lifetime claimed exceeds lifetime funded"
    );
}

/// H-02: freshly activated (warming) stake must not capture a roll of already-funded rewards.
#[tokio::test]
async fn warming_stake_cannot_snipe_funded_rewards_on_same_epoch_roll() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena_with_min_deposit(flavor, 91, 100, 5, 10, 0, 0, 1).await;

    // Honest staker matures first.
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            100,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 6).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    mature_after_activate(&mut fixture, flavor).await;

    // Fund while only honest stake is mature.
    let reward = 1_000u64;
    process_tx(
        &mut fixture.context,
        &[&fixture.funder],
        vec![fund_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.funder.pubkey(),
            fixture.funder_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            reward,
        )],
    )
    .await;

    // Sniper deposits a huge bag and activates into warming (same epoch as fund).
    let sniper = Keypair::new();
    let sniper_token = Keypair::new();
    fund(&mut fixture.context, &sniper.pubkey(), LAMPORTS_PER_SOL).await;
    create_token_account(
        &mut fixture.context,
        flavor,
        &fixture.mint.pubkey(),
        &sniper.pubkey(),
        &sniper_token,
    )
    .await;
    // Re-enable mint for test funding — mint authority was revoked; use funder transfer instead.
    process_tx(
        &mut fixture.context,
        &[&fixture.funder],
        vec![match flavor {
            TokenFlavor::Spl => spl_token::instruction::transfer(
                &flavor.program_id(),
                &fixture.funder_token.pubkey(),
                &sniper_token.pubkey(),
                &fixture.funder.pubkey(),
                &[],
                50_000,
            )
            .unwrap(),
            TokenFlavor::Token2022 => spl_token_2022::instruction::transfer(
                &flavor.program_id(),
                &fixture.funder_token.pubkey(),
                &sniper_token.pubkey(),
                &fixture.funder.pubkey(),
                &[],
                50_000,
            )
            .unwrap(),
        }],
    )
    .await;
    process_tx(
        &mut fixture.context,
        &[&sniper],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            sniper.pubkey(),
            sniper_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            50_000,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 6).await;
    process_tx(
        &mut fixture.context,
        &[&sniper],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            sniper.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;

    let config = load_config(&mut fixture.context, fixture.config).await;
    assert_eq!(config.eligible_locked, 100);
    assert_eq!(config.warming_locked, 50_000);

    // Roll while sniper is warming — distribution base is mature-only.
    advance_time(&mut fixture.context, 11).await;
    process_tx(
        &mut fixture.context,
        &[],
        vec![roll_epoch(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;

    // Honest claim gets the full funded reward.
    let before = token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![claim_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    let honest_claimed =
        token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await - before;
    assert_eq!(honest_claimed, reward);

    // Sniper has no claimable rewards from that roll.
    let sniper_pos = position_pda(&id(), &fixture.config, &sniper.pubkey()).0;
    let sniper_position = load_position(&mut fixture.context, sniper_pos).await;
    assert_eq!(sniper_position.pending_rewards, 0);
    assert!(sniper_position.warming_amount > 0 || sniper_position.eligible_amount > 0);
}

#[tokio::test]
async fn prearmed_warming_stake_cannot_sync_then_snipe_target_roll() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena_with_min_deposit(flavor, 94, 30, 5, 10, 0, 0, 1).await;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            100,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 6).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    mature_after_activate(&mut fixture, flavor).await;

    let sniper = Keypair::new();
    let sniper_token = Keypair::new();
    fund(&mut fixture.context, &sniper.pubkey(), LAMPORTS_PER_SOL).await;
    create_token_account(
        &mut fixture.context,
        flavor,
        &fixture.mint.pubkey(),
        &sniper.pubkey(),
        &sniper_token,
    )
    .await;
    process_tx(
        &mut fixture.context,
        &[&fixture.funder],
        vec![spl_token::instruction::transfer(
            &flavor.program_id(),
            &fixture.funder_token.pubkey(),
            &sniper_token.pubkey(),
            &fixture.funder.pubkey(),
            &[],
            50_000,
        )
        .unwrap()],
    )
    .await;
    process_tx(
        &mut fixture.context,
        &[&sniper],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            sniper.pubkey(),
            sniper_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            50_000,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 6).await;
    process_tx(
        &mut fixture.context,
        &[&sniper],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            sniper.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;

    // Advance one epoch without touching the sniper. Its warming stake is now
    // ready to mature but still excluded from eligible_locked.
    advance_time(&mut fixture.context, 11).await;
    process_tx(
        &mut fixture.context,
        &[],
        vec![roll_epoch(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    let config_before_fund = load_config(&mut fixture.context, fixture.config).await;
    assert_eq!(config_before_fund.eligible_locked, 100);
    assert_eq!(config_before_fund.warming_locked, 50_000);

    let reward = 1_000u64;
    process_tx(
        &mut fixture.context,
        &[&fixture.funder],
        vec![fund_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.funder.pubkey(),
            fixture.funder_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            reward,
        )],
    )
    .await;

    // Same transaction shape from the audit: sync maturity, roll, full exit.
    advance_time(&mut fixture.context, 40).await;
    process_tx(
        &mut fixture.context,
        &[&sniper],
        vec![
            activate_position(
                id(),
                fixture.authority.pubkey(),
                fixture.config_id,
                sniper.pubkey(),
                fixture.reward_pool_token.pubkey(),
                fixture.mint.pubkey(),
                flavor.program_id(),
            ),
            roll_epoch(
                id(),
                fixture.authority.pubkey(),
                fixture.config_id,
                fixture.reward_pool_token.pubkey(),
                fixture.mint.pubkey(),
                flavor.program_id(),
            ),
            withdraw(
                id(),
                fixture.authority.pubkey(),
                fixture.config_id,
                sniper.pubkey(),
                sniper_token.pubkey(),
                fixture.vault_token.pubkey(),
                fixture.reward_pool_token.pubkey(),
                fixture.mint.pubkey(),
                flavor.program_id(),
                50_000,
            ),
        ],
    )
    .await;

    let sniper_position = load_position(
        &mut fixture.context,
        position_pda(&id(), &fixture.config, &sniper.pubkey()).0,
    )
    .await;
    assert_eq!(sniper_position.total_rewards_claimed, 0);
    assert_eq!(
        token_balance(&mut fixture.context, flavor, sniper_token.pubkey()).await,
        50_000
    );

    let before_honest_claim =
        token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![claim_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    let honest_claimed = token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey())
        .await
        - before_honest_claim;
    assert_eq!(honest_claimed, reward);
}

#[tokio::test]
async fn maturing_topup_preserves_existing_eligible_rewards() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena_with_min_deposit(flavor, 95, 100, 5, 10, 0, 0, 1).await;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            100,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 6).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    mature_after_activate(&mut fixture, flavor).await;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            1,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 6).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;

    process_tx(
        &mut fixture.context,
        &[&fixture.funder],
        vec![fund_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.funder.pubkey(),
            fixture.funder_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            100,
        )],
    )
    .await;

    advance_time(&mut fixture.context, 11).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;

    let before_claim =
        token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![claim_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    let claimed = token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await
        - before_claim;
    assert_eq!(claimed, 100);
}

#[tokio::test]
async fn fractional_batches_do_not_block_full_exit_and_finalize_surplus() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena_with_min_deposit(flavor, 96, 1_000, 5, 10, 0, 0, 1).await;

    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![deposit(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            3,
        )],
    )
    .await;
    advance_time(&mut fixture.context, 6).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![activate_position(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    mature_after_activate(&mut fixture, flavor).await;

    for expected_claim in [1u64, 2u64] {
        process_tx(
            &mut fixture.context,
            &[&fixture.funder],
            vec![fund_rewards(
                id(),
                fixture.authority.pubkey(),
                fixture.config_id,
                fixture.funder.pubkey(),
                fixture.funder_token.pubkey(),
                fixture.reward_pool_token.pubkey(),
                fixture.mint.pubkey(),
                flavor.program_id(),
                2,
            )],
        )
        .await;
        let before = token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await;
        process_tx(
            &mut fixture.context,
            &[&fixture.user],
            vec![claim_rewards(
                id(),
                fixture.authority.pubkey(),
                fixture.config_id,
                fixture.user.pubkey(),
                fixture.user_token.pubkey(),
                fixture.reward_pool_token.pubkey(),
                fixture.mint.pubkey(),
                flavor.program_id(),
            )],
        )
        .await;
        let claimed =
            token_balance(&mut fixture.context, flavor, fixture.user_token.pubkey()).await - before;
        assert_eq!(claimed, expected_claim);
    }
    assert_eq!(
        token_balance(
            &mut fixture.context,
            flavor,
            fixture.reward_pool_token.pubkey()
        )
        .await,
        1
    );

    advance_time(&mut fixture.context, 1_001).await;
    process_tx(
        &mut fixture.context,
        &[&fixture.user],
        vec![withdraw(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.user.pubkey(),
            fixture.user_token.pubkey(),
            fixture.vault_token.pubkey(),
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
            3,
        )],
    )
    .await;
    assert_eq!(
        token_balance(&mut fixture.context, flavor, fixture.vault_token.pubkey()).await,
        0
    );
    assert_eq!(
        token_balance(
            &mut fixture.context,
            flavor,
            fixture.reward_pool_token.pubkey()
        )
        .await,
        1
    );

    process_tx(
        &mut fixture.context,
        &[],
        vec![finalize_rewards(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
            fixture.reward_pool_token.pubkey(),
            fixture.mint.pubkey(),
            flavor.program_id(),
        )],
    )
    .await;
    assert_eq!(
        token_balance(
            &mut fixture.context,
            flavor,
            fixture.reward_pool_token.pubkey()
        )
        .await,
        0
    );
}

/// H-03: split early exits must accumulate the same penalty as one bulk exit.
#[tokio::test]
async fn split_early_exit_penalty_matches_bulk_exit() {
    let flavor = TokenFlavor::Spl;
    let mut bulk = setup_arena_with_min_deposit(flavor, 92, 1_000, 10, 10, 5_000, 0, 2).await;
    let mut split = setup_arena_with_min_deposit(flavor, 93, 1_000, 10, 10, 5_000, 0, 2).await;
    let deposit_amount = 3u64;

    for fixture in [&mut bulk, &mut split] {
        process_tx(
            &mut fixture.context,
            &[&fixture.user],
            vec![deposit(
                id(),
                fixture.authority.pubkey(),
                fixture.config_id,
                fixture.user.pubkey(),
                fixture.user_token.pubkey(),
                fixture.vault_token.pubkey(),
                fixture.mint.pubkey(),
                flavor.program_id(),
                deposit_amount,
            )],
        )
        .await;
    }

    // Bulk: Withdraw(2) at 50% -> penalty 1, with one unit still locked.
    process_tx(
        &mut bulk.context,
        &[&bulk.user],
        vec![withdraw(
            id(),
            bulk.authority.pubkey(),
            bulk.config_id,
            bulk.user.pubkey(),
            bulk.user_token.pubkey(),
            bulk.vault_token.pubkey(),
            bulk.reward_pool_token.pubkey(),
            bulk.mint.pubkey(),
            flavor.program_id(),
            2,
        )],
    )
    .await;
    let bulk_pos = load_position(&mut bulk.context, bulk.position).await;
    assert_eq!(bulk_pos.total_penalty_paid, 1);

    // Split: Withdraw(1) + Withdraw(1) must also total penalty 1 (not 0+0)
    // while the position remains continuously open.
    process_tx(
        &mut split.context,
        &[&split.user],
        vec![withdraw(
            id(),
            split.authority.pubkey(),
            split.config_id,
            split.user.pubkey(),
            split.user_token.pubkey(),
            split.vault_token.pubkey(),
            split.reward_pool_token.pubkey(),
            split.mint.pubkey(),
            flavor.program_id(),
            1,
        )],
    )
    .await;
    let split_after_first = load_position(&mut split.context, split.position).await;
    assert_eq!(split_after_first.total_penalty_paid, 0);
    assert_eq!(split_after_first.penalty_remainder, 5_000);
    advance_time(&mut split.context, 1).await;
    process_tx(
        &mut split.context,
        &[&split.user],
        vec![withdraw(
            id(),
            split.authority.pubkey(),
            split.config_id,
            split.user.pubkey(),
            split.user_token.pubkey(),
            split.vault_token.pubkey(),
            split.reward_pool_token.pubkey(),
            split.mint.pubkey(),
            flavor.program_id(),
            1,
        )],
    )
    .await;
    let split_pos = load_position(&mut split.context, split.position).await;
    assert_eq!(split_pos.locked_amount, 1);
    assert_eq!(split_pos.penalty_remainder, 0);
    assert_eq!(split_pos.total_burned, 1);
    assert_eq!(
        split_pos.total_penalty_paid, bulk_pos.total_penalty_paid,
        "H-03: split early exits must not bypass cumulative penalty"
    );
}
