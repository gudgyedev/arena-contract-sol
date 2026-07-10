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
        activate_position, claim_rewards, config_pda, deposit, fund_rewards, initialize_config,
        position_pda, roll_epoch, vault_authority_pda, withdraw,
    },
    processor::process_instruction,
    state::{ArenaConfig, ArenaPosition, CONFIG_SIZE, POSITION_SIZE},
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
            100,
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

    let mut ix = roll_epoch(id(), fixture.authority.pubkey(), fixture.config_id);
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
        position_bump: 0,
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
async fn rejects_claim_from_zero_stake_orphan_position() {
    let flavor = TokenFlavor::Spl;
    let mut fixture = setup_arena(flavor, 12, 1_000, 10, 10, 1_000, 100).await;
    let (_, position_bump) = position_pda(&id(), &fixture.config, &fixture.user.pubkey());
    let rent = fixture.context.banks_client.get_rent().await.unwrap();
    let orphan_position = ArenaPosition {
        is_initialized: true,
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
        position_bump,
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
        )],
    )
    .await;

    let position = load_position(&mut fixture.context, fixture.position).await;
    assert_eq!(position.eligible_amount, amount);
    assert_eq!(position.pending_activation_amount, 0);

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
        &[],
        vec![roll_epoch(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
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
    assert_eq!(config.eligible_locked, 900_000);
    assert_eq!(config.pending_activation_locked, 0);
    assert_eq!(config.pending_rewards, 9_000);
    assert_eq!(config.total_penalties_collected, 10_000);
    assert_eq!(config.total_burned, 1_000);
    assert_eq!(position.total_penalty_paid, 10_000);
    assert_eq!(position.total_burned, 1_000);
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
        9_000
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
    assert_eq!(config.total_rewards_expired, reward);
    assert_eq!(config.total_penalties_collected, 100_000);
    assert_eq!(config.total_burned, 200_000);
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
            reward,
        )],
    )
    .await;
    process_tx(
        &mut fixture.context,
        &[],
        vec![roll_epoch(
            id(),
            fixture.authority.pubkey(),
            fixture.config_id,
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
    assert_eq!(config.total_burned, 5);
    assert_eq!(config.pending_rewards, 45);
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
