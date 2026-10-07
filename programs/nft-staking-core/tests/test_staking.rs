use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{
            account_info::IntoAccountInfo, clock::Clock, instruction::Instruction,
            system_instruction, system_program,
        },
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    anchor_spl::{
        associated_token::get_associated_token_address,
        token::{spl_token, TokenAccount},
    },
    litesvm::LiteSVM,
    mpl_core::{
        fetch_asset_plugin, fetch_collection_plugin,
        types::{Attributes, BurnDelegate, FreezeDelegate, PluginType},
    },
    nft_staking_core::{
        constants::{CONFIG_SEED, STAKE_SEED, TOTAL_STAKED_KEY},
        state::{StakeConfig, UserStake},
    },
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

struct TestContext {
    svm: LiteSVM,
    program_id: Pubkey,
    admin: Keypair,
    stake_config: Pubkey,
    reward_mint: Keypair,
    collection: Keypair,
}

impl TestContext {
    fn new() -> Self {
        let mut svm = LiteSVM::new();
        let program_id = nft_staking_core::id();

        // Load programs
        let core_bytes = include_bytes!("fixtures/mpl_core.so");
        svm.add_program(mpl_core::ID, core_bytes).unwrap();

        let prog_bytes = include_bytes!(concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../deploy/nft_staking_core.so"
        ));
        svm.add_program(program_id, prog_bytes).unwrap();

        // Setup admin
        let admin = Keypair::new();
        svm.airdrop(&admin.pubkey(), 20_000_000_000).unwrap();

        let (stake_config, _) = Pubkey::find_program_address(&[CONFIG_SEED], &program_id);

        // Create reward mint with stake_config as mint authority
        let reward_mint = Keypair::new();
        let rent = svm.minimum_balance_for_rent_exemption(82);
        let create_mint_ix = system_instruction::create_account(
            &admin.pubkey(),
            &reward_mint.pubkey(),
            rent,
            82,
            &anchor_spl::token::ID,
        );
        let init_mint_ix = spl_token::instruction::initialize_mint2(
            &anchor_spl::token::ID,
            &reward_mint.pubkey(),
            &stake_config,
            None,
            6,
        )
        .unwrap();

        let blockhash = svm.latest_blockhash();
        let msg = Message::new_with_blockhash(
            &[create_mint_ix, init_mint_ix],
            Some(&admin.pubkey()),
            &blockhash,
        );
        let tx = VersionedTransaction::try_new(
            VersionedMessage::Legacy(msg),
            &[&admin, &reward_mint],
        )
        .unwrap();
        svm.send_transaction(tx).unwrap();

        // Initialize StakeConfig
        let init_config_ix = Instruction::new_with_bytes(
            program_id,
            &nft_staking_core::instruction::InitializeConfig {
                points_per_sec: 10,
                burn_bonus_points: 5_000,
            }
            .data(),
            nft_staking_core::accounts::InitializeConfig {
                admin: admin.pubkey(),
                stake_config,
                reward_mint: reward_mint.pubkey(),
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        );

        let blockhash = svm.latest_blockhash();
        let msg = Message::new_with_blockhash(&[init_config_ix], Some(&admin.pubkey()), &blockhash);
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&admin]).unwrap();
        let res = svm.send_transaction(tx);
        assert!(res.is_ok(), "InitializeConfig failed: {:?}", res.err());

        // Create Collection
        let collection = Keypair::new();
        let create_col_ix = mpl_core::instructions::CreateCollectionV1Builder::new()
            .collection(collection.pubkey())
            .payer(admin.pubkey())
            .update_authority(Some(admin.pubkey()))
            .name("Turbin3 Collection".to_string())
            .uri("https://example.com/col.json".to_string())
            .instruction();

        let blockhash = svm.latest_blockhash();
        let msg = Message::new_with_blockhash(&[create_col_ix], Some(&admin.pubkey()), &blockhash);
        let tx = VersionedTransaction::try_new(
            VersionedMessage::Legacy(msg),
            &[&admin, &collection],
        )
        .unwrap();
        let res = svm.send_transaction(tx);
        assert!(res.is_ok(), "CreateCollection failed: {:?}", res.err());

        // Initialize Collection Stats
        let init_stats_ix = Instruction::new_with_bytes(
            program_id,
            &nft_staking_core::instruction::InitCollectionStats {}.data(),
            nft_staking_core::accounts::InitCollectionStats {
                admin: admin.pubkey(),
                stake_config,
                collection: collection.pubkey(),
                mpl_core_program: mpl_core::ID,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        );

        let blockhash = svm.latest_blockhash();
        let msg = Message::new_with_blockhash(&[init_stats_ix], Some(&admin.pubkey()), &blockhash);
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&admin]).unwrap();
        let res = svm.send_transaction(tx);
        assert!(res.is_ok(), "InitCollectionStats failed: {:?}", res.err());

        Self {
            svm,
            program_id,
            admin,
            stake_config,
            reward_mint,
            collection,
        }
    }

    fn create_user_and_ata(&mut self) -> (Keypair, Pubkey) {
        let user = Keypair::new();
        self.svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

        let user_ata = get_associated_token_address(&user.pubkey(), &self.reward_mint.pubkey());
        let create_ata_ix = Instruction {
            program_id: anchor_spl::associated_token::ID,
            accounts: vec![
                anchor_lang::solana_program::instruction::AccountMeta::new(user.pubkey(), true),
                anchor_lang::solana_program::instruction::AccountMeta::new(user_ata, false),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(user.pubkey(), false),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(self.reward_mint.pubkey(), false),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(system_program::ID, false),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(anchor_spl::token::ID, false),
            ],
            data: vec![],
        };

        let blockhash = self.svm.latest_blockhash();
        let msg = Message::new_with_blockhash(&[create_ata_ix], Some(&user.pubkey()), &blockhash);
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user]).unwrap();
        self.svm.send_transaction(tx).unwrap();

        (user, user_ata)
    }

    fn create_asset(&mut self, owner: &Pubkey) -> Keypair {
        let asset = Keypair::new();
        let create_asset_ix = mpl_core::instructions::CreateV1Builder::new()
            .asset(asset.pubkey())
            .collection(Some(self.collection.pubkey()))
            .authority(Some(self.admin.pubkey()))
            .payer(self.admin.pubkey())
            .owner(Some(*owner))
            .name("Turbin3 Core Asset".to_string())
            .uri("https://example.com/asset.json".to_string())
            .instruction();

        let blockhash = self.svm.latest_blockhash();
        let msg = Message::new_with_blockhash(
            &[create_asset_ix],
            Some(&self.admin.pubkey()),
            &blockhash,
        );
        let tx = VersionedTransaction::try_new(
            VersionedMessage::Legacy(msg),
            &[&self.admin, &asset],
        )
        .unwrap();
        let res = self.svm.send_transaction(tx);
        assert!(res.is_ok(), "CreateAsset failed: {:?}", res.err());

        asset
    }

    fn get_total_staked(&mut self) -> u64 {
        let mut col_account = self.svm.get_account(&self.collection.pubkey()).unwrap();
        let col_key = self.collection.pubkey();
        let col_info = (&col_key, &mut col_account).into_account_info();
        let (_, attributes, _) =
            fetch_collection_plugin::<Attributes>(&col_info, PluginType::Attributes).unwrap();
        let total_staked_attr = attributes
            .attribute_list
            .iter()
            .find(|a| a.key == TOTAL_STAKED_KEY)
            .expect("total_staked attribute must exist");
        total_staked_attr.value.parse::<u64>().unwrap()
    }

    fn get_ata_balance(&self, ata: &Pubkey) -> u64 {
        let ata_account = self.svm.get_account(ata).unwrap();
        let mut data: &[u8] = &ata_account.data;
        let token_account = TokenAccount::try_deserialize(&mut data).unwrap();
        token_account.amount
    }

    fn warp_time(&mut self, seconds: i64) {
        let mut clock = self.svm.get_sysvar::<Clock>();
        clock.unix_timestamp += seconds;
        self.svm.set_sysvar::<Clock>(&clock);
        self.svm.expire_blockhash();
    }
}

#[test]
fn test_initialize_config_and_collection_stats() {
    let mut ctx = TestContext::new();

    // Verify StakeConfig account state
    let config_account = ctx.svm.get_account(&ctx.stake_config).unwrap();
    let mut data: &[u8] = &config_account.data;
    let config_state = StakeConfig::try_deserialize(&mut data).unwrap();
    assert_eq!(config_state.admin, ctx.admin.pubkey());
    assert_eq!(config_state.reward_mint, ctx.reward_mint.pubkey());
    assert_eq!(config_state.points_per_sec, 10);
    assert_eq!(config_state.burn_bonus_points, 5_000);

    // Verify initial total_staked is 0
    assert_eq!(ctx.get_total_staked(), 0);
}

#[test]
fn test_stake_and_claim_rewards() {
    let mut ctx = TestContext::new();
    let (user, user_ata) = ctx.create_user_and_ata();
    let asset = ctx.create_asset(&user.pubkey());

    let (user_stake, _) = Pubkey::find_program_address(
        &[STAKE_SEED, asset.pubkey().as_ref()],
        &ctx.program_id,
    );

    // 1. Stake NFT
    let stake_ix = Instruction::new_with_bytes(
        ctx.program_id,
        &nft_staking_core::instruction::Stake {}.data(),
        nft_staking_core::accounts::Stake {
            owner: user.pubkey(),
            stake_config: ctx.stake_config,
            asset: asset.pubkey(),
            collection: ctx.collection.pubkey(),
            user_stake,
            mpl_core_program: mpl_core::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = ctx.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[stake_ix], Some(&user.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user]).unwrap();
    let res = ctx.svm.send_transaction(tx);
    assert!(res.is_ok(), "Stake failed: {:?}", res.err());

    // Verify UserStake PDA state
    let user_stake_acc = ctx.svm.get_account(&user_stake).unwrap();
    let mut data: &[u8] = &user_stake_acc.data;
    let user_stake_state = UserStake::try_deserialize(&mut data).unwrap();
    assert_eq!(user_stake_state.owner, user.pubkey());
    assert_eq!(user_stake_state.asset, asset.pubkey());
    assert_eq!(user_stake_state.collection, ctx.collection.pubkey());

    // Verify FreezeDelegate and BurnDelegate are active
    let mut asset_account = ctx.svm.get_account(&asset.pubkey()).unwrap();
    let asset_key = asset.pubkey();
    let asset_info = (&asset_key, &mut asset_account).into_account_info();
    let (_, freeze_delegate, _) =
        fetch_asset_plugin::<FreezeDelegate>(&asset_info, PluginType::FreezeDelegate).unwrap();
    assert!(freeze_delegate.frozen, "Asset should be frozen upon staking");

    let (_, _burn_delegate, _) =
        fetch_asset_plugin::<BurnDelegate>(&asset_info, PluginType::BurnDelegate).unwrap();

    // Verify collection total_staked incremented to 1
    assert_eq!(ctx.get_total_staked(), 1);

    // Verify initial ATA balance is 0
    assert_eq!(ctx.get_ata_balance(&user_ata), 0);

    // 2. Warp time forward by 100 seconds
    ctx.warp_time(100);

    // 3. Claim rewards without unstaking
    let claim_ix = Instruction::new_with_bytes(
        ctx.program_id,
        &nft_staking_core::instruction::ClaimRewards {}.data(),
        nft_staking_core::accounts::ClaimRewards {
            owner: user.pubkey(),
            stake_config: ctx.stake_config,
            asset: asset.pubkey(),
            user_stake,
            reward_mint: ctx.reward_mint.pubkey(),
            user_reward_ata: user_ata,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = ctx.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[claim_ix], Some(&user.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user]).unwrap();
    let res = ctx.svm.send_transaction(tx);
    assert!(res.is_ok(), "ClaimRewards failed: {:?}", res.err());

    // Verify rewards minted: 100 seconds * 10 points/sec = 1,000 tokens
    assert_eq!(ctx.get_ata_balance(&user_ata), 1_000);

    // Verify NFT remains frozen and staked
    let mut asset_account = ctx.svm.get_account(&asset.pubkey()).unwrap();
    let asset_info = (&asset_key, &mut asset_account).into_account_info();
    let (_, freeze_delegate, _) =
        fetch_asset_plugin::<FreezeDelegate>(&asset_info, PluginType::FreezeDelegate).unwrap();
    assert!(freeze_delegate.frozen, "Asset must remain frozen after claiming rewards");

    // Verify collection total_staked remains 1
    assert_eq!(ctx.get_total_staked(), 1);

    // 4. Warp time forward another 50 seconds and claim again
    ctx.warp_time(50);

    let claim_ix = Instruction::new_with_bytes(
        ctx.program_id,
        &nft_staking_core::instruction::ClaimRewards {}.data(),
        nft_staking_core::accounts::ClaimRewards {
            owner: user.pubkey(),
            stake_config: ctx.stake_config,
            asset: asset.pubkey(),
            user_stake,
            reward_mint: ctx.reward_mint.pubkey(),
            user_reward_ata: user_ata,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = ctx.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[claim_ix], Some(&user.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user]).unwrap();
    let res = ctx.svm.send_transaction(tx);
    assert!(res.is_ok(), "Second ClaimRewards failed: {:?}", res.err());

    // Total rewards: 1,000 + 50 * 10 = 1,500 tokens
    assert_eq!(ctx.get_ata_balance(&user_ata), 1_500);
}

#[test]
fn test_burn_staked_nft() {
    let mut ctx = TestContext::new();
    let (user, user_ata) = ctx.create_user_and_ata();
    let asset = ctx.create_asset(&user.pubkey());

    let (user_stake, _) = Pubkey::find_program_address(
        &[STAKE_SEED, asset.pubkey().as_ref()],
        &ctx.program_id,
    );

    // 1. Stake NFT
    let stake_ix = Instruction::new_with_bytes(
        ctx.program_id,
        &nft_staking_core::instruction::Stake {}.data(),
        nft_staking_core::accounts::Stake {
            owner: user.pubkey(),
            stake_config: ctx.stake_config,
            asset: asset.pubkey(),
            collection: ctx.collection.pubkey(),
            user_stake,
            mpl_core_program: mpl_core::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = ctx.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[stake_ix], Some(&user.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user]).unwrap();
    ctx.svm.send_transaction(tx).unwrap();

    assert_eq!(ctx.get_total_staked(), 1);

    // 2. Warp time forward by 200 seconds
    ctx.warp_time(200);

    // 3. Burn staked NFT for bonus rewards
    let burn_ix = Instruction::new_with_bytes(
        ctx.program_id,
        &nft_staking_core::instruction::BurnStakedNft {}.data(),
        nft_staking_core::accounts::BurnStakedNft {
            owner: user.pubkey(),
            stake_config: ctx.stake_config,
            asset: asset.pubkey(),
            collection: ctx.collection.pubkey(),
            user_stake,
            reward_mint: ctx.reward_mint.pubkey(),
            user_reward_ata: user_ata,
            mpl_core_program: mpl_core::ID,
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = ctx.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[burn_ix], Some(&user.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user]).unwrap();
    let res = ctx.svm.send_transaction(tx);
    assert!(res.is_ok(), "BurnStakedNft failed: {:?}", res.err());

    // Expected rewards: 200 sec * 10 points/sec + 5,000 burn bonus = 7,000 tokens
    assert_eq!(ctx.get_ata_balance(&user_ata), 7_000);

    // Verify asset account is burned (closed or reset to Key::Uninitialized)
    let burned_asset_acc = ctx.svm.get_account(&asset.pubkey());
    assert!(
        burned_asset_acc.is_none()
            || (burned_asset_acc.as_ref().unwrap().data.len() == 1
                && burned_asset_acc.as_ref().unwrap().data[0] == 0),
        "Asset account must be uninitialized or closed"
    );

    // Verify UserStake account is closed
    let user_stake_acc = ctx.svm.get_account(&user_stake);
    assert!(
        user_stake_acc.is_none() || user_stake_acc.as_ref().unwrap().lamports == 0,
        "UserStake account must be closed"
    );

    // Verify collection total_staked decremented back to 0
    assert_eq!(ctx.get_total_staked(), 0);
}

#[test]
fn test_unstake() {
    let mut ctx = TestContext::new();
    let (user, user_ata) = ctx.create_user_and_ata();
    let asset = ctx.create_asset(&user.pubkey());

    let (user_stake, _) = Pubkey::find_program_address(
        &[STAKE_SEED, asset.pubkey().as_ref()],
        &ctx.program_id,
    );

    // 1. Stake NFT
    let stake_ix = Instruction::new_with_bytes(
        ctx.program_id,
        &nft_staking_core::instruction::Stake {}.data(),
        nft_staking_core::accounts::Stake {
            owner: user.pubkey(),
            stake_config: ctx.stake_config,
            asset: asset.pubkey(),
            collection: ctx.collection.pubkey(),
            user_stake,
            mpl_core_program: mpl_core::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = ctx.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[stake_ix], Some(&user.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user]).unwrap();
    ctx.svm.send_transaction(tx).unwrap();

    assert_eq!(ctx.get_total_staked(), 1);

    // 2. Warp time forward by 80 seconds
    ctx.warp_time(80);

    // 3. Unstake NFT
    let unstake_ix = Instruction::new_with_bytes(
        ctx.program_id,
        &nft_staking_core::instruction::Unstake {}.data(),
        nft_staking_core::accounts::Unstake {
            owner: user.pubkey(),
            stake_config: ctx.stake_config,
            asset: asset.pubkey(),
            collection: ctx.collection.pubkey(),
            user_stake,
            reward_mint: ctx.reward_mint.pubkey(),
            user_reward_ata: user_ata,
            mpl_core_program: mpl_core::ID,
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = ctx.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[unstake_ix], Some(&user.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user]).unwrap();
    let res = ctx.svm.send_transaction(tx);
    assert!(res.is_ok(), "Unstake failed: {:?}", res.err());

    // Expected rewards: 80 sec * 10 points/sec = 800 tokens
    assert_eq!(ctx.get_ata_balance(&user_ata), 800);

    // Verify FreezeDelegate and BurnDelegate are removed
    let mut asset_account = ctx.svm.get_account(&asset.pubkey()).unwrap();
    let asset_key = asset.pubkey();
    let asset_info = (&asset_key, &mut asset_account).into_account_info();
    let freeze_res =
        fetch_asset_plugin::<FreezeDelegate>(&asset_info, PluginType::FreezeDelegate);
    assert!(freeze_res.is_err(), "FreezeDelegate plugin should be removed");

    let burn_res = fetch_asset_plugin::<BurnDelegate>(&asset_info, PluginType::BurnDelegate);
    assert!(burn_res.is_err(), "BurnDelegate plugin should be removed");

    // Verify UserStake account is closed
    let user_stake_acc = ctx.svm.get_account(&user_stake);
    assert!(
        user_stake_acc.is_none() || user_stake_acc.as_ref().unwrap().lamports == 0,
        "UserStake account must be closed"
    );

    // Verify collection total_staked decremented back to 0
    assert_eq!(ctx.get_total_staked(), 0);
}

#[test]
fn test_unauthorized_claim_fails() {
    let mut ctx = TestContext::new();
    let (user, _user_ata) = ctx.create_user_and_ata();
    let (attacker, attacker_ata) = ctx.create_user_and_ata();
    let asset = ctx.create_asset(&user.pubkey());

    let (user_stake, _) = Pubkey::find_program_address(
        &[STAKE_SEED, asset.pubkey().as_ref()],
        &ctx.program_id,
    );

    // User stakes
    let stake_ix = Instruction::new_with_bytes(
        ctx.program_id,
        &nft_staking_core::instruction::Stake {}.data(),
        nft_staking_core::accounts::Stake {
            owner: user.pubkey(),
            stake_config: ctx.stake_config,
            asset: asset.pubkey(),
            collection: ctx.collection.pubkey(),
            user_stake,
            mpl_core_program: mpl_core::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = ctx.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[stake_ix], Some(&user.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user]).unwrap();
    ctx.svm.send_transaction(tx).unwrap();

    ctx.warp_time(100);

    // Attacker tries to claim rewards on user's staked NFT
    let claim_ix = Instruction::new_with_bytes(
        ctx.program_id,
        &nft_staking_core::instruction::ClaimRewards {}.data(),
        nft_staking_core::accounts::ClaimRewards {
            owner: attacker.pubkey(),
            stake_config: ctx.stake_config,
            asset: asset.pubkey(),
            user_stake,
            reward_mint: ctx.reward_mint.pubkey(),
            user_reward_ata: attacker_ata,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = ctx.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[claim_ix], Some(&attacker.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&attacker]).unwrap();
    let res = ctx.svm.send_transaction(tx);
    assert!(res.is_err(), "Attacker claim must fail due to has_one constraint");
}

