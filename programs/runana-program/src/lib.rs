use anchor_lang::{
    prelude::*,
    solana_program::sysvar::instructions::{
        load_current_index_checked, load_instruction_at_checked,
    },
    AccountsExit,
};
use solana_program::{ed25519_program, hash::hashv};

declare_id!("CaUejpPZoNjFmSrkfbazrjBUXE8FK1c2Hoz64NFsTfLm");

const PROGRAM_CONFIG_SEED: &[u8] = b"program_config";
const SIGNATURE_SCHEME_ED25519_RAW_V1: u8 = 0;
const SIGNATURE_SCHEME_WALLET_TEXT_V1: u8 = 1;
const HEX_LOWER: &[u8; 16] = b"0123456789abcdef";
const CHARACTER_SEED: &[u8] = b"character";
const CHARACTER_STATS_SEED: &[u8] = b"character_stats";
const CHARACTER_WORLD_PROGRESS_SEED: &[u8] = b"character_world_progress";
const CHARACTER_ZONE_PROGRESS_SEED: &[u8] = b"character_zone_progress";
const CHARACTER_BATCH_CURSOR_SEED: &[u8] = b"character_batch_cursor";
const ZONE_REGISTRY_SEED: &[u8] = b"zone_registry";
const ZONE_ENEMY_SET_SEED: &[u8] = b"zone_enemy_set";
const ENEMY_ARCHETYPE_SEED: &[u8] = b"enemy_archetype";
const SEASON_POLICY_SEED: &[u8] = b"season_policy";

const ACCOUNT_VERSION_V1: u8 = 1;
const CLUSTER_ID_LOCALNET: u8 = 1;
const ZONE_STATE_UNLOCKED: u8 = 1;
const ZONE_STATE_CLEARED: u8 = 2;
const ZONE_PAGE_WIDTH: u16 = 256;
const THROUGHPUT_CAP_PER_MINUTE: u64 = 20;
const MAX_ZONE_ENEMY_SET_MEMBERS: usize = 64;

const ED25519_SIGNATURE_COUNT_OFFSET: usize = 0;
const ED25519_OFFSETS_START: usize = 2;
const ED25519_OFFSETS_SIZE: usize = 14;
const ED25519_PUBKEY_SIZE: usize = 32;
const ED25519_SIGNATURE_SIZE: usize = 64;
const ED25519_SELF_INSTRUCTION_INDEX: u16 = u16::MAX;

#[program]
pub mod runana_program {
    use super::*;

    pub fn initialize_program_config(
        ctx: Context<InitializeProgramConfig>,
        args: InitializeProgramConfigArgs,
    ) -> Result<()> {
        let config = &mut ctx.accounts.program_config;
        config.version = ACCOUNT_VERSION_V1;
        config.bump = ctx.bumps.program_config;
        config.admin_authority = ctx.accounts.admin_authority.key();
        config.trusted_server_signer = args.trusted_server_signer;
        config.settlement_paused = args.settlement_paused;
        config.max_battles_per_batch = args.max_battles_per_batch;
        config.max_histogram_entries_per_batch = args.max_histogram_entries_per_batch;
        config.updated_at_slot = Clock::get()?.slot;
        Ok(())
    }

    pub fn initialize_zone_registry(
        ctx: Context<InitializeZoneRegistry>,
        args: InitializeZoneRegistryArgs,
    ) -> Result<()> {
        require!(
            args.exp_multiplier_den > 0,
            SettlementError::InvalidZoneConfig
        );

        let zone_registry = &mut ctx.accounts.zone_registry;
        zone_registry.version = ACCOUNT_VERSION_V1;
        zone_registry.bump = ctx.bumps.zone_registry;
        zone_registry.zone_id = args.zone_id;
        zone_registry.exp_multiplier_num = args.exp_multiplier_num;
        zone_registry.exp_multiplier_den = args.exp_multiplier_den;
        Ok(())
    }

    pub fn initialize_zone_enemy_set(
        ctx: Context<InitializeZoneEnemySet>,
        args: InitializeZoneEnemySetArgs,
    ) -> Result<()> {
        verify_zone_enemy_set_members(&args.allowed_enemy_archetype_ids)?;

        let zone_enemy_set = &mut ctx.accounts.zone_enemy_set;
        zone_enemy_set.version = ACCOUNT_VERSION_V1;
        zone_enemy_set.bump = ctx.bumps.zone_enemy_set;
        zone_enemy_set.zone_id = args.zone_id;
        zone_enemy_set.allowed_enemy_archetype_ids = args.allowed_enemy_archetype_ids;
        Ok(())
    }

    pub fn update_zone_enemy_set(
        ctx: Context<UpdateZoneEnemySet>,
        args: UpdateZoneEnemySetArgs,
    ) -> Result<()> {
        verify_zone_enemy_set_members(&args.allowed_enemy_archetype_ids)?;

        let zone_enemy_set = &mut ctx.accounts.zone_enemy_set;
        require!(
            zone_enemy_set.zone_id == args.zone_id,
            SettlementError::ZoneEnemySetMismatch
        );
        zone_enemy_set.allowed_enemy_archetype_ids = args.allowed_enemy_archetype_ids;
        Ok(())
    }

    pub fn initialize_enemy_archetype_registry(
        ctx: Context<InitializeEnemyArchetypeRegistry>,
        args: InitializeEnemyArchetypeRegistryArgs,
    ) -> Result<()> {
        let enemy_archetype_registry = &mut ctx.accounts.enemy_archetype_registry;
        enemy_archetype_registry.version = ACCOUNT_VERSION_V1;
        enemy_archetype_registry.bump = ctx.bumps.enemy_archetype_registry;
        enemy_archetype_registry.enemy_archetype_id = args.enemy_archetype_id;
        enemy_archetype_registry.exp_reward_base = args.exp_reward_base;
        Ok(())
    }

    pub fn initialize_season_policy(
        ctx: Context<InitializeSeasonPolicy>,
        args: InitializeSeasonPolicyArgs,
    ) -> Result<()> {
        require!(
            args.season_start_ts < args.season_end_ts
                && args.season_end_ts <= args.commit_grace_end_ts,
            SettlementError::InvalidSeasonPolicy
        );

        let season_policy = &mut ctx.accounts.season_policy;
        season_policy.version = ACCOUNT_VERSION_V1;
        season_policy.bump = ctx.bumps.season_policy;
        season_policy.season_id = args.season_id;
        season_policy.season_start_ts = args.season_start_ts;
        season_policy.season_end_ts = args.season_end_ts;
        season_policy.commit_grace_end_ts = args.commit_grace_end_ts;
        season_policy.updated_at_slot = Clock::get()?.slot;
        Ok(())
    }

    pub fn create_character(
        ctx: Context<CreateCharacter>,
        args: CreateCharacterArgs,
    ) -> Result<()> {
        let clock = Clock::get()?;
        require!(
            clock.unix_timestamp >= 0,
            SettlementError::InvalidClockTimestamp
        );
        let character_creation_ts = clock.unix_timestamp as u64;
        require!(
            ctx.accounts.season_policy.season_start_ts < ctx.accounts.season_policy.season_end_ts
                && ctx.accounts.season_policy.season_end_ts
                    <= ctx.accounts.season_policy.commit_grace_end_ts,
            SettlementError::InvalidSeasonPolicy
        );
        require!(
            ctx.accounts.season_policy.season_start_ts <= character_creation_ts
                && character_creation_ts <= ctx.accounts.season_policy.season_end_ts,
            SettlementError::SeasonWindowClosed
        );

        let initial_page_index = args.initial_unlocked_zone_id / ZONE_PAGE_WIDTH;
        require!(
            ctx.accounts.character_zone_progress_page.page_index == initial_page_index,
            SettlementError::InvalidZoneProgressPage
        );

        let character_root_key = ctx.accounts.character_root.key();
        let genesis_state_hash = compute_genesis_state_hash(character_root_key, args.character_id);

        let character_root = &mut ctx.accounts.character_root;
        character_root.version = ACCOUNT_VERSION_V1;
        character_root.bump = ctx.bumps.character_root;
        character_root.authority = ctx.accounts.authority.key();
        character_root.character_id = args.character_id;
        character_root.character_creation_ts = character_creation_ts;

        let character_stats = &mut ctx.accounts.character_stats;
        character_stats.version = ACCOUNT_VERSION_V1;
        character_stats.bump = ctx.bumps.character_stats;
        character_stats.character_root = character_root_key;
        character_stats.level = 1;
        character_stats.total_exp = 0;

        let character_world_progress = &mut ctx.accounts.character_world_progress;
        character_world_progress.version = ACCOUNT_VERSION_V1;
        character_world_progress.bump = ctx.bumps.character_world_progress;
        character_world_progress.character_root = character_root_key;
        character_world_progress.highest_unlocked_zone_id = args.initial_unlocked_zone_id;
        character_world_progress.highest_cleared_zone_id = 0;

        let character_zone_progress_page = &mut ctx.accounts.character_zone_progress_page;
        character_zone_progress_page.version = ACCOUNT_VERSION_V1;
        character_zone_progress_page.bump = ctx.bumps.character_zone_progress_page;
        character_zone_progress_page.character_root = character_root_key;
        character_zone_progress_page.page_index = initial_page_index;
        character_zone_progress_page.zone_states = [0_u8; ZONE_PAGE_WIDTH as usize];
        character_zone_progress_page.zone_states
            [(args.initial_unlocked_zone_id % ZONE_PAGE_WIDTH) as usize] = ZONE_STATE_UNLOCKED;

        let cursor = &mut ctx.accounts.character_settlement_batch_cursor;
        cursor.version = ACCOUNT_VERSION_V1;
        cursor.bump = ctx.bumps.character_settlement_batch_cursor;
        cursor.character_root = character_root_key;
        cursor.last_committed_end_nonce = 0;
        cursor.last_committed_state_hash = genesis_state_hash;
        cursor.last_committed_batch_id = 0;
        cursor.last_committed_battle_ts = character_creation_ts;
        cursor.last_committed_season_id = ctx.accounts.season_policy.season_id;
        cursor.updated_at_slot = clock.slot;

        Ok(())
    }

    pub fn initialize_character_zone_progress_page(
        ctx: Context<InitializeCharacterZoneProgressPage>,
        args: InitializeCharacterZoneProgressPageArgs,
    ) -> Result<()> {
        let character_zone_progress_page = &mut ctx.accounts.character_zone_progress_page;
        character_zone_progress_page.version = ACCOUNT_VERSION_V1;
        character_zone_progress_page.bump = ctx.bumps.character_zone_progress_page;
        character_zone_progress_page.character_root = ctx.accounts.character_root.key();
        character_zone_progress_page.page_index = args.page_index;
        character_zone_progress_page.zone_states = [0_u8; ZONE_PAGE_WIDTH as usize];

        Ok(())
    }

    pub fn apply_battle_settlement_batch_v1<'info>(
        ctx: Context<'_, '_, 'info, 'info, ApplyBattleSettlementBatchV1<'info>>,
        args: ApplyBattleSettlementBatchV1Args,
    ) -> Result<()> {
        let mut remaining_accounts = load_settlement_remaining_accounts(&ctx, &args.payload)?;

        verify_canonical_account_addresses(&ctx)?;
        verify_character_binding(
            &ctx,
            &args.payload,
            &remaining_accounts.additional_zone_progress_pages,
        )?;
        verify_zone_progress_account_envelope(
            &ctx,
            &args.payload,
            &remaining_accounts.additional_zone_progress_pages,
        )?;
        verify_program_controls(&ctx.accounts.program_config)?;
        verify_batch_policy_limits(&ctx.accounts.program_config, &args.payload)?;
        verify_nonce_range(&args.payload)?;
        verify_histogram_integrity(&args.payload)?;
        verify_batch_hash(&args.payload)?;
        verify_batch_continuity(
            &ctx.accounts.character_settlement_batch_cursor,
            &args.payload,
        )?;
        verify_ed25519_preinstructions(&ctx, &args.payload)?;
        verify_time_season_and_throughput(
            &ctx.accounts.character_root,
            &ctx.accounts.character_settlement_batch_cursor,
            &ctx.accounts.season_policy,
            &args.payload,
        )?;
        verify_zone_progress_delta(
            &args.payload,
            &ctx.accounts.character_world_progress,
            &ctx.accounts.character_zone_progress_page,
            &remaining_accounts.additional_zone_progress_pages,
        )?;
        verify_world_eligibility(
            &args.payload,
            &ctx.accounts.character_zone_progress_page,
            &remaining_accounts.additional_zone_progress_pages,
        )?;
        verify_zone_enemy_legality(
            &args.payload,
            &remaining_accounts.zone_registries,
            &remaining_accounts.zone_enemy_sets,
            &remaining_accounts.enemy_archetype_registries,
        )?;

        let exp_delta = derive_exp_delta(
            &args.payload,
            &remaining_accounts.zone_registries,
            &remaining_accounts.enemy_archetype_registries,
        )?;
        apply_zone_progress_delta(
            &args.payload,
            &mut ctx.accounts.character_zone_progress_page,
            &mut remaining_accounts.additional_zone_progress_pages,
            &mut ctx.accounts.character_world_progress,
        )?;
        persist_additional_zone_progress_pages(
            remaining_accounts.additional_zone_progress_pages,
            ctx.program_id,
        )?;

        let character_stats = &mut ctx.accounts.character_stats;
        character_stats.total_exp = character_stats
            .total_exp
            .checked_add(u64::from(exp_delta))
            .ok_or_else(|| error!(SettlementError::ArithmeticOverflow))?;

        let cursor = &mut ctx.accounts.character_settlement_batch_cursor;
        cursor.last_committed_end_nonce = args.payload.end_nonce;
        cursor.last_committed_state_hash = args.payload.end_state_hash;
        cursor.last_committed_batch_id = args.payload.batch_id;
        cursor.last_committed_battle_ts = args.payload.last_battle_ts;
        cursor.last_committed_season_id = args.payload.season_id;
        cursor.updated_at_slot = Clock::get()?.slot;

        msg!(
            "settlement_applied batch_id={} battle_count={} exp_delta={}",
            args.payload.batch_id,
            args.payload.battle_count,
            exp_delta
        );

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeProgramConfig<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub admin_authority: Signer<'info>,
    #[account(
        init,
        payer = payer,
        seeds = [PROGRAM_CONFIG_SEED],
        bump,
        space = ProgramConfigAccount::LEN,
    )]
    pub program_config: Account<'info, ProgramConfigAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(args: InitializeZoneRegistryArgs)]
pub struct InitializeZoneRegistry<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub admin_authority: Signer<'info>,
    #[account(
        seeds = [PROGRAM_CONFIG_SEED],
        bump = program_config.bump,
        constraint = program_config.admin_authority == admin_authority.key() @ SettlementError::UnauthorizedAdmin,
    )]
    pub program_config: Account<'info, ProgramConfigAccount>,
    #[account(
        init,
        payer = payer,
        seeds = [ZONE_REGISTRY_SEED, &args.zone_id.to_le_bytes()],
        bump,
        space = ZoneRegistryAccount::LEN,
    )]
    pub zone_registry: Account<'info, ZoneRegistryAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(args: InitializeZoneEnemySetArgs)]
pub struct InitializeZoneEnemySet<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub admin_authority: Signer<'info>,
    #[account(
        seeds = [PROGRAM_CONFIG_SEED],
        bump = program_config.bump,
        constraint = program_config.admin_authority == admin_authority.key() @ SettlementError::UnauthorizedAdmin,
    )]
    pub program_config: Account<'info, ProgramConfigAccount>,
    #[account(
        init,
        payer = payer,
        seeds = [ZONE_ENEMY_SET_SEED, &args.zone_id.to_le_bytes()],
        bump,
        space = ZoneEnemySetAccount::LEN,
    )]
    pub zone_enemy_set: Account<'info, ZoneEnemySetAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(args: UpdateZoneEnemySetArgs)]
pub struct UpdateZoneEnemySet<'info> {
    #[account(mut)]
    pub admin_authority: Signer<'info>,
    #[account(
        seeds = [PROGRAM_CONFIG_SEED],
        bump = program_config.bump,
        constraint = program_config.admin_authority == admin_authority.key() @ SettlementError::UnauthorizedAdmin,
    )]
    pub program_config: Account<'info, ProgramConfigAccount>,
    #[account(
        mut,
        seeds = [ZONE_ENEMY_SET_SEED, &args.zone_id.to_le_bytes()],
        bump = zone_enemy_set.bump,
    )]
    pub zone_enemy_set: Account<'info, ZoneEnemySetAccount>,
}

#[derive(Accounts)]
#[instruction(args: InitializeEnemyArchetypeRegistryArgs)]
pub struct InitializeEnemyArchetypeRegistry<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub admin_authority: Signer<'info>,
    #[account(
        seeds = [PROGRAM_CONFIG_SEED],
        bump = program_config.bump,
        constraint = program_config.admin_authority == admin_authority.key() @ SettlementError::UnauthorizedAdmin,
    )]
    pub program_config: Account<'info, ProgramConfigAccount>,
    #[account(
        init,
        payer = payer,
        seeds = [ENEMY_ARCHETYPE_SEED, &args.enemy_archetype_id.to_le_bytes()],
        bump,
        space = EnemyArchetypeRegistryAccount::LEN,
    )]
    pub enemy_archetype_registry: Account<'info, EnemyArchetypeRegistryAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(args: InitializeSeasonPolicyArgs)]
pub struct InitializeSeasonPolicy<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub admin_authority: Signer<'info>,
    #[account(
        seeds = [PROGRAM_CONFIG_SEED],
        bump = program_config.bump,
        constraint = program_config.admin_authority == admin_authority.key() @ SettlementError::UnauthorizedAdmin,
    )]
    pub program_config: Account<'info, ProgramConfigAccount>,
    #[account(
        init,
        payer = payer,
        seeds = [SEASON_POLICY_SEED, &args.season_id.to_le_bytes()],
        bump,
        space = SeasonPolicyAccount::LEN,
    )]
    pub season_policy: Account<'info, SeasonPolicyAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(args: CreateCharacterArgs)]
pub struct CreateCharacter<'info> {
    #[account(
        mut,
        constraint = payer.key() == authority.key() @ SettlementError::PlayerMustSelfFund
    )]
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
    #[account(
        seeds = [SEASON_POLICY_SEED, &season_policy.season_id.to_le_bytes()],
        bump = season_policy.bump,
    )]
    pub season_policy: Account<'info, SeasonPolicyAccount>,
    #[account(
        init,
        payer = payer,
        seeds = [CHARACTER_SEED, authority.key().as_ref(), &args.character_id],
        bump,
        space = CharacterRootAccount::LEN,
    )]
    pub character_root: Account<'info, CharacterRootAccount>,
    #[account(
        init,
        payer = payer,
        seeds = [CHARACTER_STATS_SEED, character_root.key().as_ref()],
        bump,
        space = CharacterStatsAccount::LEN,
    )]
    pub character_stats: Account<'info, CharacterStatsAccount>,
    #[account(
        init,
        payer = payer,
        seeds = [CHARACTER_WORLD_PROGRESS_SEED, character_root.key().as_ref()],
        bump,
        space = CharacterWorldProgressAccount::LEN,
    )]
    pub character_world_progress: Account<'info, CharacterWorldProgressAccount>,
    #[account(
        init,
        payer = payer,
        seeds = [
            CHARACTER_ZONE_PROGRESS_SEED,
            character_root.key().as_ref(),
            &(args.initial_unlocked_zone_id / ZONE_PAGE_WIDTH).to_le_bytes(),
        ],
        bump,
        space = CharacterZoneProgressPageAccount::LEN,
    )]
    pub character_zone_progress_page: Account<'info, CharacterZoneProgressPageAccount>,
    #[account(
        init,
        payer = payer,
        seeds = [CHARACTER_BATCH_CURSOR_SEED, character_root.key().as_ref()],
        bump,
        space = CharacterSettlementBatchCursorAccount::LEN,
    )]
    pub character_settlement_batch_cursor: Account<'info, CharacterSettlementBatchCursorAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(args: InitializeCharacterZoneProgressPageArgs)]
pub struct InitializeCharacterZoneProgressPage<'info> {
    #[account(
        mut,
        constraint = payer.key() == authority.key() @ SettlementError::PlayerMustSelfFund
    )]
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
    #[account(
        seeds = [CHARACTER_SEED, authority.key().as_ref(), &character_root.character_id],
        bump = character_root.bump,
        constraint = character_root.authority == authority.key() @ SettlementError::PlayerAuthorityMismatch,
    )]
    pub character_root: Account<'info, CharacterRootAccount>,
    #[account(
        init,
        payer = payer,
        seeds = [
            CHARACTER_ZONE_PROGRESS_SEED,
            character_root.key().as_ref(),
            &args.page_index.to_le_bytes(),
        ],
        bump,
        space = CharacterZoneProgressPageAccount::LEN,
    )]
    pub character_zone_progress_page: Account<'info, CharacterZoneProgressPageAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ApplyBattleSettlementBatchV1<'info> {
    /// CHECK: settlement permit subject; verified via ed25519 permit contents and character ownership.
    pub player_authority: UncheckedAccount<'info>,
    /// CHECK: sysvar instructions account is validated by address and parsed at runtime.
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub program_config: Account<'info, ProgramConfigAccount>,
    pub character_root: Account<'info, CharacterRootAccount>,
    #[account(mut)]
    pub character_stats: Account<'info, CharacterStatsAccount>,
    #[account(mut)]
    pub character_world_progress: Account<'info, CharacterWorldProgressAccount>,
    #[account(mut)]
    pub character_zone_progress_page: Account<'info, CharacterZoneProgressPageAccount>,
    pub season_policy: Account<'info, SeasonPolicyAccount>,
    #[account(mut)]
    pub character_settlement_batch_cursor: Account<'info, CharacterSettlementBatchCursorAccount>,
}

#[account]
pub struct ProgramConfigAccount {
    pub version: u8,
    pub bump: u8,
    pub admin_authority: Pubkey,
    pub trusted_server_signer: Pubkey,
    pub settlement_paused: bool,
    pub max_battles_per_batch: u16,
    pub max_histogram_entries_per_batch: u16,
    pub updated_at_slot: u64,
}

impl ProgramConfigAccount {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 32 + 1 + 2 + 2 + 8;
}

#[account]
pub struct CharacterRootAccount {
    pub version: u8,
    pub bump: u8,
    pub authority: Pubkey,
    pub character_id: [u8; 16],
    pub character_creation_ts: u64,
}

impl CharacterRootAccount {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 16 + 8;
}

#[account]
pub struct CharacterStatsAccount {
    pub version: u8,
    pub bump: u8,
    pub character_root: Pubkey,
    pub level: u16,
    pub total_exp: u64,
}

impl CharacterStatsAccount {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 2 + 8;
}

#[account]
pub struct CharacterWorldProgressAccount {
    pub version: u8,
    pub bump: u8,
    pub character_root: Pubkey,
    pub highest_unlocked_zone_id: u16,
    pub highest_cleared_zone_id: u16,
}

impl CharacterWorldProgressAccount {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 2 + 2;
}

#[account]
pub struct CharacterZoneProgressPageAccount {
    pub version: u8,
    pub bump: u8,
    pub character_root: Pubkey,
    pub page_index: u16,
    pub zone_states: [u8; ZONE_PAGE_WIDTH as usize],
}

impl CharacterZoneProgressPageAccount {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 2 + (ZONE_PAGE_WIDTH as usize);
}

#[account]
pub struct ZoneRegistryAccount {
    pub version: u8,
    pub bump: u8,
    pub zone_id: u16,
    pub exp_multiplier_num: u16,
    pub exp_multiplier_den: u16,
}

impl ZoneRegistryAccount {
    pub const LEN: usize = 8 + 1 + 1 + 2 + 2 + 2;
}

#[account]
pub struct ZoneEnemySetAccount {
    pub version: u8,
    pub bump: u8,
    pub zone_id: u16,
    pub allowed_enemy_archetype_ids: Vec<u16>,
}

impl ZoneEnemySetAccount {
    pub const LEN: usize = 8 + 1 + 1 + 2 + 4 + (MAX_ZONE_ENEMY_SET_MEMBERS * 2);
}

#[account]
pub struct EnemyArchetypeRegistryAccount {
    pub version: u8,
    pub bump: u8,
    pub enemy_archetype_id: u16,
    pub exp_reward_base: u32,
}

impl EnemyArchetypeRegistryAccount {
    pub const LEN: usize = 8 + 1 + 1 + 2 + 4;
}

#[account]
pub struct SeasonPolicyAccount {
    pub version: u8,
    pub bump: u8,
    pub season_id: u32,
    pub season_start_ts: u64,
    pub season_end_ts: u64,
    pub commit_grace_end_ts: u64,
    pub updated_at_slot: u64,
}

impl SeasonPolicyAccount {
    pub const LEN: usize = 8 + 1 + 1 + 4 + 8 + 8 + 8 + 8;
}

#[account]
pub struct CharacterSettlementBatchCursorAccount {
    pub version: u8,
    pub bump: u8,
    pub character_root: Pubkey,
    pub last_committed_end_nonce: u64,
    pub last_committed_state_hash: [u8; 32],
    pub last_committed_batch_id: u64,
    pub last_committed_battle_ts: u64,
    pub last_committed_season_id: u32,
    pub updated_at_slot: u64,
}

impl CharacterSettlementBatchCursorAccount {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 8 + 32 + 8 + 8 + 4 + 8;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct InitializeProgramConfigArgs {
    pub trusted_server_signer: Pubkey,
    pub settlement_paused: bool,
    pub max_battles_per_batch: u16,
    pub max_histogram_entries_per_batch: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct InitializeZoneRegistryArgs {
    pub zone_id: u16,
    pub exp_multiplier_num: u16,
    pub exp_multiplier_den: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct InitializeZoneEnemySetArgs {
    pub zone_id: u16,
    pub allowed_enemy_archetype_ids: Vec<u16>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct UpdateZoneEnemySetArgs {
    pub zone_id: u16,
    pub allowed_enemy_archetype_ids: Vec<u16>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct InitializeEnemyArchetypeRegistryArgs {
    pub enemy_archetype_id: u16,
    pub exp_reward_base: u32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct InitializeSeasonPolicyArgs {
    pub season_id: u32,
    pub season_start_ts: u64,
    pub season_end_ts: u64,
    pub commit_grace_end_ts: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct CreateCharacterArgs {
    pub character_id: [u8; 16],
    pub initial_unlocked_zone_id: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct InitializeCharacterZoneProgressPageArgs {
    pub page_index: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct ApplyBattleSettlementBatchV1Args {
    pub payload: SettlementBatchPayloadV1,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct SettlementBatchPayloadV1 {
    pub character_id: [u8; 16],
    pub batch_id: u64,
    pub start_nonce: u64,
    pub end_nonce: u64,
    pub battle_count: u16,
    pub start_state_hash: [u8; 32],
    pub end_state_hash: [u8; 32],
    pub zone_progress_delta: Vec<ZoneProgressDeltaEntry>,
    pub encounter_histogram: Vec<EncounterCountEntry>,
    pub optional_loadout_revision: Option<u32>,
    pub batch_hash: [u8; 32],
    pub first_battle_ts: u64,
    pub last_battle_ts: u64,
    pub season_id: u32,
    pub schema_version: u16,
    pub signature_scheme: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct EncounterCountEntry {
    pub zone_id: u16,
    pub enemy_archetype_id: u16,
    pub count: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct ZoneProgressDeltaEntry {
    pub zone_id: u16,
    pub new_state: u8,
}

#[derive(AnchorSerialize)]
struct SettlementBatchPayloadPreimageV1 {
    character_id: [u8; 16],
    batch_id: u64,
    start_nonce: u64,
    end_nonce: u64,
    battle_count: u16,
    first_battle_ts: u64,
    last_battle_ts: u64,
    season_id: u32,
    start_state_hash: [u8; 32],
    end_state_hash: [u8; 32],
    zone_progress_delta: Vec<ZoneProgressDeltaEntry>,
    encounter_histogram: Vec<EncounterCountEntry>,
    optional_loadout_revision: Option<u32>,
    schema_version: u16,
    signature_scheme: u8,
}

impl From<&SettlementBatchPayloadV1> for SettlementBatchPayloadPreimageV1 {
    fn from(payload: &SettlementBatchPayloadV1) -> Self {
        Self {
            character_id: payload.character_id,
            batch_id: payload.batch_id,
            start_nonce: payload.start_nonce,
            end_nonce: payload.end_nonce,
            battle_count: payload.battle_count,
            first_battle_ts: payload.first_battle_ts,
            last_battle_ts: payload.last_battle_ts,
            season_id: payload.season_id,
            start_state_hash: payload.start_state_hash,
            end_state_hash: payload.end_state_hash,
            zone_progress_delta: payload.zone_progress_delta.clone(),
            encounter_histogram: payload.encounter_histogram.clone(),
            optional_loadout_revision: payload.optional_loadout_revision,
            schema_version: payload.schema_version,
            signature_scheme: payload.signature_scheme,
        }
    }
}

fn compute_genesis_state_hash(character_root_pubkey: Pubkey, character_id: [u8; 16]) -> [u8; 32] {
    hashv(&[
        character_root_pubkey.as_ref(),
        &character_id,
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
    ])
    .to_bytes()
}

fn verify_canonical_account_addresses(ctx: &Context<ApplyBattleSettlementBatchV1>) -> Result<()> {
    let program_id = ctx.program_id;
    let character_root = &ctx.accounts.character_root;

    let (expected_program_config, _) =
        Pubkey::find_program_address(&[PROGRAM_CONFIG_SEED], program_id);
    require_keys_eq!(
        ctx.accounts.program_config.key(),
        expected_program_config,
        SettlementError::InvalidProgramConfigPda
    );

    let (expected_character_root, _) = Pubkey::find_program_address(
        &[
            CHARACTER_SEED,
            character_root.authority.as_ref(),
            &character_root.character_id,
        ],
        program_id,
    );
    require_keys_eq!(
        character_root.key(),
        expected_character_root,
        SettlementError::InvalidCharacterPda
    );

    let (expected_character_stats, _) = Pubkey::find_program_address(
        &[CHARACTER_STATS_SEED, character_root.key().as_ref()],
        program_id,
    );
    require_keys_eq!(
        ctx.accounts.character_stats.key(),
        expected_character_stats,
        SettlementError::InvalidCharacterStatsPda
    );

    let (expected_character_world_progress, _) = Pubkey::find_program_address(
        &[CHARACTER_WORLD_PROGRESS_SEED, character_root.key().as_ref()],
        program_id,
    );
    require_keys_eq!(
        ctx.accounts.character_world_progress.key(),
        expected_character_world_progress,
        SettlementError::InvalidCharacterWorldProgressPda
    );

    let (expected_character_cursor, _) = Pubkey::find_program_address(
        &[CHARACTER_BATCH_CURSOR_SEED, character_root.key().as_ref()],
        program_id,
    );
    require_keys_eq!(
        ctx.accounts.character_settlement_batch_cursor.key(),
        expected_character_cursor,
        SettlementError::InvalidCharacterCursorPda
    );

    let (expected_season_policy, _) = Pubkey::find_program_address(
        &[
            SEASON_POLICY_SEED,
            &ctx.accounts.season_policy.season_id.to_le_bytes(),
        ],
        program_id,
    );
    require_keys_eq!(
        ctx.accounts.season_policy.key(),
        expected_season_policy,
        SettlementError::InvalidSeasonPolicyPda
    );

    let (expected_zone_progress_page, _) = Pubkey::find_program_address(
        &[
            CHARACTER_ZONE_PROGRESS_SEED,
            character_root.key().as_ref(),
            &ctx.accounts
                .character_zone_progress_page
                .page_index
                .to_le_bytes(),
        ],
        program_id,
    );
    require_keys_eq!(
        ctx.accounts.character_zone_progress_page.key(),
        expected_zone_progress_page,
        SettlementError::InvalidZoneProgressPagePda
    );

    Ok(())
}

fn verify_program_controls(program_config: &ProgramConfigAccount) -> Result<()> {
    require!(
        !program_config.settlement_paused,
        SettlementError::SettlementPaused
    );
    Ok(())
}

fn verify_batch_policy_limits(
    program_config: &ProgramConfigAccount,
    payload: &SettlementBatchPayloadV1,
) -> Result<()> {
    require!(
        payload.battle_count <= program_config.max_battles_per_batch,
        SettlementError::BatchBattleCountLimitExceeded
    );
    require!(
        payload.encounter_histogram.len()
            <= usize::from(program_config.max_histogram_entries_per_batch),
        SettlementError::HistogramEntryLimitExceeded
    );
    Ok(())
}

fn verify_character_binding(
    ctx: &Context<ApplyBattleSettlementBatchV1>,
    payload: &SettlementBatchPayloadV1,
    additional_zone_progress_pages: &[LoadedZoneProgressPage],
) -> Result<()> {
    let character_root = &ctx.accounts.character_root;

    require!(
        character_root.character_id == payload.character_id,
        SettlementError::CharacterIdMismatch
    );
    require_keys_eq!(
        character_root.authority,
        ctx.accounts.player_authority.key(),
        SettlementError::PlayerAuthorityMismatch
    );
    require_keys_eq!(
        ctx.accounts.character_stats.character_root,
        character_root.key(),
        SettlementError::CharacterAccountBindingMismatch
    );
    require_keys_eq!(
        ctx.accounts.character_world_progress.character_root,
        character_root.key(),
        SettlementError::CharacterAccountBindingMismatch
    );
    require_keys_eq!(
        ctx.accounts.character_zone_progress_page.character_root,
        character_root.key(),
        SettlementError::CharacterAccountBindingMismatch
    );
    require_keys_eq!(
        ctx.accounts
            .character_settlement_batch_cursor
            .character_root,
        character_root.key(),
        SettlementError::CharacterAccountBindingMismatch
    );
    for page in additional_zone_progress_pages {
        require_keys_eq!(
            page.data.character_root,
            character_root.key(),
            SettlementError::CharacterAccountBindingMismatch
        );
    }

    Ok(())
}

struct LoadedSettlementRemainingAccounts<'info> {
    additional_zone_progress_pages: Vec<LoadedZoneProgressPage<'info>>,
    zone_registries: Vec<ZoneRegistryAccount>,
    zone_enemy_sets: Vec<ZoneEnemySetAccount>,
    enemy_archetype_registries: Vec<EnemyArchetypeRegistryAccount>,
}

fn load_settlement_remaining_accounts<'info>(
    ctx: &Context<'_, '_, 'info, 'info, ApplyBattleSettlementBatchV1<'info>>,
    payload: &SettlementBatchPayloadV1,
) -> Result<LoadedSettlementRemainingAccounts<'info>> {
    let referenced_page_indices = referenced_zone_page_indices(payload);
    let referenced_zone_ids = referenced_zone_ids(payload);
    let referenced_enemy_ids = referenced_enemy_archetype_ids(payload);
    let mut cursor = 0usize;

    let additional_zone_progress_pages = load_additional_zone_progress_pages(
        ctx,
        referenced_page_indices.get(1..).unwrap_or(&[]),
        &mut cursor,
    )?;
    let zone_registries = load_zone_registry_accounts(ctx, &referenced_zone_ids, &mut cursor)?;
    let zone_enemy_sets = load_zone_enemy_set_accounts(ctx, &referenced_zone_ids, &mut cursor)?;
    let enemy_archetype_registries =
        load_enemy_archetype_registry_accounts(ctx, &referenced_enemy_ids, &mut cursor)?;

    require!(
        cursor == ctx.remaining_accounts.len(),
        SettlementError::UnexpectedSettlementRemainingAccounts
    );

    Ok(LoadedSettlementRemainingAccounts {
        additional_zone_progress_pages,
        zone_registries,
        zone_enemy_sets,
        enemy_archetype_registries,
    })
}

fn load_additional_zone_progress_pages<'info>(
    ctx: &Context<'_, '_, 'info, 'info, ApplyBattleSettlementBatchV1<'info>>,
    expected_page_indices: &[u16],
    cursor: &mut usize,
) -> Result<Vec<LoadedZoneProgressPage<'info>>> {
    let mut pages = Vec::with_capacity(expected_page_indices.len());

    for expected_page_index in expected_page_indices {
        let account_info = ctx
            .remaining_accounts
            .get(*cursor)
            .ok_or_else(|| error!(SettlementError::MissingZoneProgressPageAccount))?;
        let expected_zone_progress_page = zone_progress_page_pda(
            ctx.program_id,
            &ctx.accounts.character_root.key(),
            *expected_page_index,
        );
        require_keys_eq!(
            account_info.key(),
            expected_zone_progress_page,
            SettlementError::ZoneProgressPageAccountOrderMismatch
        );

        let account = Account::<CharacterZoneProgressPageAccount>::try_from(account_info)?;
        pages.push(LoadedZoneProgressPage {
            info: account_info.clone(),
            data: account.into_inner(),
        });
        *cursor += 1;
    }

    Ok(pages)
}

fn load_zone_registry_accounts<'info>(
    ctx: &Context<'_, '_, 'info, 'info, ApplyBattleSettlementBatchV1<'info>>,
    expected_zone_ids: &[u16],
    cursor: &mut usize,
) -> Result<Vec<ZoneRegistryAccount>> {
    let mut zone_registries = Vec::with_capacity(expected_zone_ids.len());

    for zone_id in expected_zone_ids {
        let account_info = ctx
            .remaining_accounts
            .get(*cursor)
            .ok_or_else(|| error!(SettlementError::MissingZoneRegistryAccount))?;
        let expected_zone_registry = zone_registry_pda(ctx.program_id, *zone_id);
        if account_info.key() != expected_zone_registry {
            let expected_present_later = ctx.remaining_accounts[*cursor + 1..]
                .iter()
                .any(|account| account.key() == expected_zone_registry);
            if expected_present_later {
                return err!(SettlementError::SettlementRemainingAccountOrderMismatch);
            }
            return err!(SettlementError::MissingZoneRegistryAccount);
        }

        let account = Account::<ZoneRegistryAccount>::try_from(account_info)?;
        require!(
            account.zone_id == *zone_id,
            SettlementError::InvalidZoneRegistryPda
        );
        zone_registries.push(account.into_inner());
        *cursor += 1;
    }

    Ok(zone_registries)
}

fn load_zone_enemy_set_accounts<'info>(
    ctx: &Context<'_, '_, 'info, 'info, ApplyBattleSettlementBatchV1<'info>>,
    expected_zone_ids: &[u16],
    cursor: &mut usize,
) -> Result<Vec<ZoneEnemySetAccount>> {
    let mut zone_enemy_sets = Vec::with_capacity(expected_zone_ids.len());

    for zone_id in expected_zone_ids {
        let account_info = ctx
            .remaining_accounts
            .get(*cursor)
            .ok_or_else(|| error!(SettlementError::MissingZoneEnemySetAccount))?;
        let expected_zone_enemy_set = zone_enemy_set_pda(ctx.program_id, *zone_id);
        if account_info.key() != expected_zone_enemy_set {
            let expected_present_later = ctx.remaining_accounts[*cursor + 1..]
                .iter()
                .any(|account| account.key() == expected_zone_enemy_set);
            if expected_present_later {
                return err!(SettlementError::SettlementRemainingAccountOrderMismatch);
            }
            return err!(SettlementError::MissingZoneEnemySetAccount);
        }

        let account = Account::<ZoneEnemySetAccount>::try_from(account_info)?;
        require!(
            account.zone_id == *zone_id,
            SettlementError::InvalidZoneEnemySetPda
        );
        verify_zone_enemy_set_members(&account.allowed_enemy_archetype_ids)?;
        zone_enemy_sets.push(account.into_inner());
        *cursor += 1;
    }

    Ok(zone_enemy_sets)
}

fn load_enemy_archetype_registry_accounts<'info>(
    ctx: &Context<'_, '_, 'info, 'info, ApplyBattleSettlementBatchV1<'info>>,
    expected_enemy_ids: &[u16],
    cursor: &mut usize,
) -> Result<Vec<EnemyArchetypeRegistryAccount>> {
    let mut enemy_archetypes = Vec::with_capacity(expected_enemy_ids.len());

    for enemy_archetype_id in expected_enemy_ids {
        let account_info = ctx
            .remaining_accounts
            .get(*cursor)
            .ok_or_else(|| error!(SettlementError::MissingEnemyArchetypeRegistryAccount))?;
        let expected_enemy_archetype =
            enemy_archetype_registry_pda(ctx.program_id, *enemy_archetype_id);
        if account_info.key() != expected_enemy_archetype {
            let expected_present_later = ctx.remaining_accounts[*cursor + 1..]
                .iter()
                .any(|account| account.key() == expected_enemy_archetype);
            if expected_present_later {
                return err!(SettlementError::SettlementRemainingAccountOrderMismatch);
            }
            return err!(SettlementError::MissingEnemyArchetypeRegistryAccount);
        }

        let account = Account::<EnemyArchetypeRegistryAccount>::try_from(account_info)?;
        require!(
            account.enemy_archetype_id == *enemy_archetype_id,
            SettlementError::InvalidEnemyArchetypePda
        );
        enemy_archetypes.push(account.into_inner());
        *cursor += 1;
    }

    Ok(enemy_archetypes)
}

fn verify_zone_progress_account_envelope(
    ctx: &Context<ApplyBattleSettlementBatchV1>,
    payload: &SettlementBatchPayloadV1,
    additional_zone_progress_pages: &[LoadedZoneProgressPage],
) -> Result<()> {
    let referenced_page_indices = referenced_zone_page_indices(payload);
    require!(
        !referenced_page_indices.is_empty(),
        SettlementError::MissingZoneProgressPageAccount
    );
    require!(
        referenced_page_indices[0] == ctx.accounts.character_zone_progress_page.page_index,
        SettlementError::ZoneProgressPageAccountOrderMismatch
    );
    require!(
        referenced_page_indices.len() == additional_zone_progress_pages.len() + 1,
        SettlementError::MissingZoneProgressPageAccount
    );

    verify_zone_progress_page_account(
        ctx.program_id,
        &ctx.accounts.character_root.key(),
        &ctx.accounts.character_zone_progress_page.to_account_info(),
        &ctx.accounts.character_zone_progress_page,
        referenced_page_indices[0],
    )?;

    for (page, expected_page_index) in additional_zone_progress_pages
        .iter()
        .zip(referenced_page_indices.iter().copied().skip(1))
    {
        verify_zone_progress_page_account(
            ctx.program_id,
            &ctx.accounts.character_root.key(),
            &page.info,
            &page.data,
            expected_page_index,
        )?;
    }

    verify_world_progress_summary_consistency(
        &ctx.accounts.character_world_progress,
        &ctx.accounts.character_zone_progress_page,
        additional_zone_progress_pages,
    )?;

    Ok(())
}

fn verify_zone_progress_page_account(
    program_id: &Pubkey,
    character_root_key: &Pubkey,
    account_info: &AccountInfo,
    page: &CharacterZoneProgressPageAccount,
    expected_page_index: u16,
) -> Result<()> {
    require!(
        account_info.is_writable,
        SettlementError::ZoneProgressPageMustBeWritable
    );
    require!(
        page.page_index == expected_page_index,
        SettlementError::ZoneProgressPageAccountOrderMismatch
    );

    let (expected_zone_progress_page, _) = Pubkey::find_program_address(
        &[
            CHARACTER_ZONE_PROGRESS_SEED,
            character_root_key.as_ref(),
            &expected_page_index.to_le_bytes(),
        ],
        program_id,
    );
    require_keys_eq!(
        account_info.key(),
        expected_zone_progress_page,
        SettlementError::InvalidZoneProgressPagePda
    );

    Ok(())
}

fn referenced_zone_page_indices(payload: &SettlementBatchPayloadV1) -> Vec<u16> {
    let mut page_indices = Vec::new();

    for entry in &payload.encounter_histogram {
        push_unique_page_index(&mut page_indices, entry.zone_id / ZONE_PAGE_WIDTH);
    }
    for entry in &payload.zone_progress_delta {
        push_unique_page_index(&mut page_indices, entry.zone_id / ZONE_PAGE_WIDTH);
    }

    page_indices.sort_unstable();
    page_indices
}

fn referenced_zone_ids(payload: &SettlementBatchPayloadV1) -> Vec<u16> {
    let mut zone_ids = Vec::new();
    for entry in &payload.encounter_histogram {
        if !zone_ids.contains(&entry.zone_id) {
            zone_ids.push(entry.zone_id);
        }
    }
    zone_ids.sort_unstable();
    zone_ids
}

fn referenced_enemy_archetype_ids(payload: &SettlementBatchPayloadV1) -> Vec<u16> {
    let mut enemy_ids = Vec::new();
    for entry in &payload.encounter_histogram {
        if !enemy_ids.contains(&entry.enemy_archetype_id) {
            enemy_ids.push(entry.enemy_archetype_id);
        }
    }
    enemy_ids.sort_unstable();
    enemy_ids
}

fn push_unique_page_index(page_indices: &mut Vec<u16>, page_index: u16) {
    if !page_indices.contains(&page_index) {
        page_indices.push(page_index);
    }
}

fn zone_registry_pda(program_id: &Pubkey, zone_id: u16) -> Pubkey {
    Pubkey::find_program_address(&[ZONE_REGISTRY_SEED, &zone_id.to_le_bytes()], program_id).0
}

fn zone_enemy_set_pda(program_id: &Pubkey, zone_id: u16) -> Pubkey {
    Pubkey::find_program_address(&[ZONE_ENEMY_SET_SEED, &zone_id.to_le_bytes()], program_id).0
}

fn enemy_archetype_registry_pda(program_id: &Pubkey, enemy_archetype_id: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[ENEMY_ARCHETYPE_SEED, &enemy_archetype_id.to_le_bytes()],
        program_id,
    )
    .0
}

fn zone_progress_page_pda(program_id: &Pubkey, character_root: &Pubkey, page_index: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[
            CHARACTER_ZONE_PROGRESS_SEED,
            character_root.as_ref(),
            &page_index.to_le_bytes(),
        ],
        program_id,
    )
    .0
}

fn verify_zone_enemy_set_members(enemy_ids: &[u16]) -> Result<()> {
    require!(
        enemy_ids.len() <= MAX_ZONE_ENEMY_SET_MEMBERS,
        SettlementError::InvalidZoneEnemySet
    );

    for pair in enemy_ids.windows(2) {
        require!(pair[0] < pair[1], SettlementError::InvalidZoneEnemySet);
    }

    Ok(())
}

fn verify_world_progress_summary_consistency(
    character_world_progress: &CharacterWorldProgressAccount,
    primary_zone_progress_page: &CharacterZoneProgressPageAccount,
    additional_zone_progress_pages: &[LoadedZoneProgressPage],
) -> Result<()> {
    require!(
        character_world_progress.highest_cleared_zone_id
            <= character_world_progress.highest_unlocked_zone_id,
        SettlementError::SummaryPageInconsistency
    );

    let mut highest_unlocked_from_pages = 0_u16;
    let mut highest_cleared_from_pages = 0_u16;

    accumulate_page_summary_bounds(
        primary_zone_progress_page,
        &mut highest_unlocked_from_pages,
        &mut highest_cleared_from_pages,
    )?;
    for page in additional_zone_progress_pages {
        accumulate_page_summary_bounds(
            &page.data,
            &mut highest_unlocked_from_pages,
            &mut highest_cleared_from_pages,
        )?;
    }

    require!(
        character_world_progress.highest_unlocked_zone_id >= highest_unlocked_from_pages
            && character_world_progress.highest_cleared_zone_id >= highest_cleared_from_pages,
        SettlementError::SummaryPageInconsistency
    );

    Ok(())
}

fn accumulate_page_summary_bounds(
    page: &CharacterZoneProgressPageAccount,
    highest_unlocked_from_pages: &mut u16,
    highest_cleared_from_pages: &mut u16,
) -> Result<()> {
    for (offset, state) in page.zone_states.iter().copied().enumerate() {
        require!(
            state <= ZONE_STATE_CLEARED,
            SettlementError::InvalidZoneProgressState
        );

        let zone_id = page
            .page_index
            .checked_mul(ZONE_PAGE_WIDTH)
            .and_then(|page_start| page_start.checked_add(offset as u16))
            .ok_or_else(|| error!(SettlementError::ArithmeticOverflow))?;

        if state >= ZONE_STATE_UNLOCKED {
            *highest_unlocked_from_pages = (*highest_unlocked_from_pages).max(zone_id);
        }
        if state >= ZONE_STATE_CLEARED {
            *highest_cleared_from_pages = (*highest_cleared_from_pages).max(zone_id);
        }
    }

    Ok(())
}

fn verify_ed25519_preinstructions(
    ctx: &Context<ApplyBattleSettlementBatchV1>,
    payload: &SettlementBatchPayloadV1,
) -> Result<()> {
    let instructions_sysvar = ctx.accounts.instructions_sysvar.to_account_info();
    let current_index = load_current_index_checked(&instructions_sysvar)
        .map_err(|_| error!(SettlementError::InvalidInstructionsSysvar))?
        as usize;

    require!(
        current_index >= 2,
        SettlementError::MissingEd25519Preinstructions
    );

    let server_ix = load_instruction_at_checked(current_index - 2, &instructions_sysvar)
        .map_err(|_| error!(SettlementError::InvalidInstructionsSysvar))?;
    let player_ix = load_instruction_at_checked(current_index - 1, &instructions_sysvar)
        .map_err(|_| error!(SettlementError::InvalidInstructionsSysvar))?;

    require_keys_eq!(
        server_ix.program_id,
        ed25519_program::ID,
        SettlementError::InvalidEd25519InstructionOrder
    );
    require_keys_eq!(
        player_ix.program_id,
        ed25519_program::ID,
        SettlementError::InvalidEd25519InstructionOrder
    );

    let expected_server_message = canonical_server_attestation_message(
        ctx.program_id,
        CLUSTER_ID_LOCALNET,
        ctx.accounts.character_root.key(),
        payload,
    );
    let expected_player_message = canonical_player_authorization_message(
        ctx.program_id,
        CLUSTER_ID_LOCALNET,
        ctx.accounts.player_authority.key(),
        ctx.accounts.character_root.key(),
        payload.batch_hash,
        payload.batch_id,
        payload.signature_scheme,
    )?;

    let server_ix_payload = parse_ed25519_instruction_payload(&server_ix.data)?;
    let player_ix_payload = parse_ed25519_instruction_payload(&player_ix.data)?;
    let expected_server_signer = ctx.accounts.program_config.trusted_server_signer;
    let expected_player_signer = ctx.accounts.player_authority.key();

    let server_signer_matches = server_ix_payload.signer_pubkey == expected_server_signer.as_ref();
    let player_signer_matches = player_ix_payload.signer_pubkey == expected_player_signer.as_ref();

    if !server_signer_matches
        && !player_signer_matches
        && server_ix_payload.signer_pubkey == expected_player_signer.as_ref()
        && player_ix_payload.signer_pubkey == expected_server_signer.as_ref()
    {
        return err!(SettlementError::InvalidEd25519InstructionOrder);
    }

    require!(
        server_signer_matches && server_ix_payload.message == expected_server_message.as_slice(),
        SettlementError::ServerAttestationMismatch
    );
    require!(
        player_signer_matches && player_ix_payload.message == expected_player_message.as_slice(),
        SettlementError::PlayerAuthorizationMismatch
    );

    Ok(())
}

struct ParsedEd25519InstructionPayload<'a> {
    signer_pubkey: &'a [u8],
    message: &'a [u8],
}

struct LoadedZoneProgressPage<'info> {
    info: AccountInfo<'info>,
    data: CharacterZoneProgressPageAccount,
}

fn parse_ed25519_instruction_payload(data: &[u8]) -> Result<ParsedEd25519InstructionPayload<'_>> {
    require!(
        data.len() >= ED25519_OFFSETS_START + ED25519_OFFSETS_SIZE,
        SettlementError::InvalidEd25519InstructionData
    );
    require!(
        data[ED25519_SIGNATURE_COUNT_OFFSET] == 1,
        SettlementError::InvalidEd25519InstructionData
    );

    let signature_instruction_index = read_u16_le(data, ED25519_OFFSETS_START + 2)
        .ok_or_else(|| error!(SettlementError::InvalidEd25519InstructionData))?;
    let public_key_offset = read_u16_le(data, ED25519_OFFSETS_START + 4)
        .ok_or_else(|| error!(SettlementError::InvalidEd25519InstructionData))?;
    let public_key_instruction_index = read_u16_le(data, ED25519_OFFSETS_START + 6)
        .ok_or_else(|| error!(SettlementError::InvalidEd25519InstructionData))?;
    let message_data_offset = read_u16_le(data, ED25519_OFFSETS_START + 8)
        .ok_or_else(|| error!(SettlementError::InvalidEd25519InstructionData))?;
    let message_data_size = read_u16_le(data, ED25519_OFFSETS_START + 10)
        .ok_or_else(|| error!(SettlementError::InvalidEd25519InstructionData))?;
    let message_instruction_index = read_u16_le(data, ED25519_OFFSETS_START + 12)
        .ok_or_else(|| error!(SettlementError::InvalidEd25519InstructionData))?;

    require!(
        signature_instruction_index == ED25519_SELF_INSTRUCTION_INDEX,
        SettlementError::InvalidEd25519InstructionData
    );
    require!(
        public_key_instruction_index == ED25519_SELF_INSTRUCTION_INDEX,
        SettlementError::InvalidEd25519InstructionData
    );
    require!(
        message_instruction_index == ED25519_SELF_INSTRUCTION_INDEX,
        SettlementError::InvalidEd25519InstructionData
    );

    let public_key_offset = public_key_offset as usize;
    let message_data_offset = message_data_offset as usize;
    let message_data_size = message_data_size as usize;
    let public_key_end = public_key_offset
        .checked_add(ED25519_PUBKEY_SIZE)
        .ok_or_else(|| error!(SettlementError::InvalidEd25519InstructionData))?;
    let message_end = message_data_offset
        .checked_add(message_data_size)
        .ok_or_else(|| error!(SettlementError::InvalidEd25519InstructionData))?;

    require!(
        public_key_end <= data.len(),
        SettlementError::InvalidEd25519InstructionData
    );
    require!(
        message_end <= data.len(),
        SettlementError::InvalidEd25519InstructionData
    );
    require!(
        public_key_end
            .checked_add(ED25519_SIGNATURE_SIZE)
            .ok_or_else(|| error!(SettlementError::InvalidEd25519InstructionData))?
            <= data.len(),
        SettlementError::InvalidEd25519InstructionData
    );

    Ok(ParsedEd25519InstructionPayload {
        signer_pubkey: &data[public_key_offset..public_key_end],
        message: &data[message_data_offset..message_end],
    })
}

fn read_u16_le(data: &[u8], offset: usize) -> Option<u16> {
    let bytes: [u8; 2] = data.get(offset..offset + 2)?.try_into().ok()?;
    Some(u16::from_le_bytes(bytes))
}

fn verify_nonce_range(payload: &SettlementBatchPayloadV1) -> Result<()> {
    let expected_battle_count = payload
        .end_nonce
        .checked_sub(payload.start_nonce)
        .and_then(|delta| delta.checked_add(1))
        .ok_or_else(|| error!(SettlementError::InvalidNonceRange))?;

    require!(
        expected_battle_count == u64::from(payload.battle_count),
        SettlementError::InvalidNonceRange
    );

    Ok(())
}

fn verify_histogram_integrity(payload: &SettlementBatchPayloadV1) -> Result<()> {
    for (index, entry) in payload.encounter_histogram.iter().enumerate() {
        require!(
            entry.count > 0,
            SettlementError::ZeroEncounterHistogramEntry
        );

        let is_duplicate = payload.encounter_histogram[..index].iter().any(|prior| {
            prior.zone_id == entry.zone_id && prior.enemy_archetype_id == entry.enemy_archetype_id
        });
        require!(
            !is_duplicate,
            SettlementError::DuplicateEncounterHistogramEntry
        );
    }

    let histogram_total = payload
        .encounter_histogram
        .iter()
        .try_fold(0_u64, |acc, entry| {
            acc.checked_add(u64::from(entry.count))
                .ok_or_else(|| error!(SettlementError::ArithmeticOverflow))
        })?;

    require!(
        histogram_total == u64::from(payload.battle_count),
        SettlementError::HistogramCountMismatch
    );

    Ok(())
}

fn verify_world_eligibility(
    payload: &SettlementBatchPayloadV1,
    primary_zone_progress_page: &CharacterZoneProgressPageAccount,
    additional_zone_progress_pages: &[LoadedZoneProgressPage],
) -> Result<()> {
    for entry in &payload.encounter_histogram {
        let effective_zone_state = effective_zone_state_after_batch(
            entry.zone_id,
            payload,
            primary_zone_progress_page,
            additional_zone_progress_pages,
        )?;
        require!(
            effective_zone_state >= ZONE_STATE_UNLOCKED,
            SettlementError::IllegalLockedZoneReference
        );
    }

    Ok(())
}

fn effective_zone_state_after_batch(
    zone_id: u16,
    payload: &SettlementBatchPayloadV1,
    primary_zone_progress_page: &CharacterZoneProgressPageAccount,
    additional_zone_progress_pages: &[LoadedZoneProgressPage],
) -> Result<u8> {
    let mut effective_state = zone_state(
        zone_id,
        primary_zone_progress_page,
        additional_zone_progress_pages,
    )?;

    for delta in &payload.zone_progress_delta {
        if delta.zone_id == zone_id && delta.new_state > effective_state {
            effective_state = delta.new_state;
        }
    }

    Ok(effective_state)
}

fn verify_zone_progress_delta(
    payload: &SettlementBatchPayloadV1,
    character_world_progress: &CharacterWorldProgressAccount,
    primary_zone_progress_page: &CharacterZoneProgressPageAccount,
    additional_zone_progress_pages: &[LoadedZoneProgressPage],
) -> Result<()> {
    let mut seen_zone_ids = Vec::new();

    for entry in &payload.zone_progress_delta {
        require!(
            !seen_zone_ids.contains(&entry.zone_id),
            SettlementError::DuplicateZoneProgressDelta
        );
        seen_zone_ids.push(entry.zone_id);

        require!(
            entry.new_state == ZONE_STATE_UNLOCKED || entry.new_state == ZONE_STATE_CLEARED,
            SettlementError::InvalidZoneProgressDelta
        );

        let prior_state = zone_state(
            entry.zone_id,
            primary_zone_progress_page,
            additional_zone_progress_pages,
        )?;
        require!(
            entry.new_state >= prior_state,
            SettlementError::InvalidZoneProgressDelta
        );

        let is_allowed_transition = match prior_state {
            0 => entry.new_state == ZONE_STATE_UNLOCKED,
            ZONE_STATE_UNLOCKED => {
                entry.new_state == ZONE_STATE_UNLOCKED || entry.new_state == ZONE_STATE_CLEARED
            }
            ZONE_STATE_CLEARED => entry.new_state == ZONE_STATE_CLEARED,
            _ => false,
        };
        require!(
            is_allowed_transition,
            SettlementError::InvalidZoneProgressDelta
        );
    }

    verify_world_progress_summary_consistency(
        character_world_progress,
        primary_zone_progress_page,
        additional_zone_progress_pages,
    )?;

    Ok(())
}

fn zone_state(
    zone_id: u16,
    primary_zone_progress_page: &CharacterZoneProgressPageAccount,
    additional_zone_progress_pages: &[LoadedZoneProgressPage],
) -> Result<u8> {
    let expected_page_index = zone_id / ZONE_PAGE_WIDTH;

    if primary_zone_progress_page.page_index == expected_page_index {
        return Ok(primary_zone_progress_page.zone_states[(zone_id % ZONE_PAGE_WIDTH) as usize]);
    }

    let page = additional_zone_progress_pages
        .iter()
        .find(|page| page.data.page_index == expected_page_index)
        .ok_or_else(|| error!(SettlementError::MissingZoneProgressPageAccount))?;

    Ok(page.data.zone_states[(zone_id % ZONE_PAGE_WIDTH) as usize])
}

fn verify_zone_enemy_legality(
    payload: &SettlementBatchPayloadV1,
    zone_registries: &[ZoneRegistryAccount],
    zone_enemy_sets: &[ZoneEnemySetAccount],
    enemy_archetype_registries: &[EnemyArchetypeRegistryAccount],
) -> Result<()> {
    for entry in &payload.encounter_histogram {
        let _zone_registry = zone_registry_for_entry(zone_registries, entry.zone_id)?;
        let zone_enemy_set = zone_enemy_set_for_entry(zone_enemy_sets, entry.zone_id)?;
        let _enemy_archetype = enemy_archetype_registry_for_entry(
            enemy_archetype_registries,
            entry.enemy_archetype_id,
        )?;

        require!(
            zone_enemy_set
                .allowed_enemy_archetype_ids
                .binary_search(&entry.enemy_archetype_id)
                .is_ok(),
            SettlementError::IllegalZoneEnemyPair
        );
    }

    Ok(())
}

fn zone_registry_for_entry(
    zone_registries: &[ZoneRegistryAccount],
    zone_id: u16,
) -> Result<&ZoneRegistryAccount> {
    zone_registries
        .iter()
        .find(|zone_registry| zone_registry.zone_id == zone_id)
        .ok_or_else(|| error!(SettlementError::MissingZoneRegistryAccount))
}

fn zone_enemy_set_for_entry(
    zone_enemy_sets: &[ZoneEnemySetAccount],
    zone_id: u16,
) -> Result<&ZoneEnemySetAccount> {
    zone_enemy_sets
        .iter()
        .find(|zone_enemy_set| zone_enemy_set.zone_id == zone_id)
        .ok_or_else(|| error!(SettlementError::MissingZoneEnemySetAccount))
}

fn enemy_archetype_registry_for_entry(
    enemy_archetype_registries: &[EnemyArchetypeRegistryAccount],
    enemy_archetype_id: u16,
) -> Result<&EnemyArchetypeRegistryAccount> {
    enemy_archetype_registries
        .iter()
        .find(|enemy| enemy.enemy_archetype_id == enemy_archetype_id)
        .ok_or_else(|| error!(SettlementError::MissingEnemyArchetypeRegistryAccount))
}

fn verify_batch_hash(payload: &SettlementBatchPayloadV1) -> Result<()> {
    let preimage = SettlementBatchPayloadPreimageV1::from(payload);
    let preimage_bytes = preimage
        .try_to_vec()
        .map_err(|_| error!(SettlementError::PreimageSerializationFailed))?;
    let recomputed = hashv(&[&preimage_bytes]).to_bytes();

    require!(
        recomputed == payload.batch_hash,
        SettlementError::BatchHashMismatch
    );

    Ok(())
}

fn verify_batch_continuity(
    cursor: &CharacterSettlementBatchCursorAccount,
    payload: &SettlementBatchPayloadV1,
) -> Result<()> {
    let expected_start_nonce = cursor
        .last_committed_end_nonce
        .checked_add(1)
        .ok_or_else(|| error!(SettlementError::ArithmeticOverflow))?;
    let expected_batch_id = cursor
        .last_committed_batch_id
        .checked_add(1)
        .ok_or_else(|| error!(SettlementError::ArithmeticOverflow))?;

    require!(
        payload.start_nonce == expected_start_nonce,
        SettlementError::InvalidStartNonce
    );
    require!(
        payload.batch_id == expected_batch_id,
        SettlementError::InvalidBatchSequence
    );
    require!(
        payload.start_state_hash == cursor.last_committed_state_hash,
        SettlementError::StartStateHashMismatch
    );

    Ok(())
}

fn verify_time_season_and_throughput(
    character_root: &CharacterRootAccount,
    cursor: &CharacterSettlementBatchCursorAccount,
    season_policy: &SeasonPolicyAccount,
    payload: &SettlementBatchPayloadV1,
) -> Result<()> {
    require!(
        season_policy.season_id == payload.season_id,
        SettlementError::SeasonPolicyMismatch
    );
    require!(
        season_policy.season_start_ts < season_policy.season_end_ts
            && season_policy.season_end_ts <= season_policy.commit_grace_end_ts,
        SettlementError::InvalidSeasonPolicy
    );
    require!(
        payload.first_battle_ts >= character_root.character_creation_ts,
        SettlementError::PreCharacterTimestamp
    );
    require!(
        payload.first_battle_ts >= cursor.last_committed_battle_ts,
        SettlementError::BattleTimestampRegression
    );
    require!(
        payload.last_battle_ts >= payload.first_battle_ts,
        SettlementError::BattleTimestampRegression
    );
    require!(
        payload.season_id >= cursor.last_committed_season_id,
        SettlementError::SeasonRegression
    );
    require!(
        payload.first_battle_ts >= season_policy.season_start_ts,
        SettlementError::SeasonWindowClosed
    );
    require!(
        payload.last_battle_ts <= season_policy.season_end_ts,
        SettlementError::SeasonWindowClosed
    );

    let current_unix_timestamp = Clock::get()?.unix_timestamp;
    require!(
        current_unix_timestamp >= 0,
        SettlementError::InvalidClockTimestamp
    );
    require!(
        (current_unix_timestamp as u64) <= season_policy.commit_grace_end_ts,
        SettlementError::SeasonWindowClosed
    );

    let interval_seconds = payload
        .last_battle_ts
        .checked_sub(payload.first_battle_ts)
        .ok_or_else(|| error!(SettlementError::BattleTimestampRegression))?;
    let allowed_battles = interval_seconds
        .checked_mul(THROUGHPUT_CAP_PER_MINUTE)
        .ok_or_else(|| error!(SettlementError::ArithmeticOverflow))?
        / 60
        + 1;
    require!(
        u64::from(payload.battle_count) <= allowed_battles,
        SettlementError::ThroughputExceeded
    );

    Ok(())
}

fn derive_exp_delta(
    payload: &SettlementBatchPayloadV1,
    zone_registries: &[ZoneRegistryAccount],
    enemy_archetype_registries: &[EnemyArchetypeRegistryAccount],
) -> Result<u32> {
    let mut total_exp = 0_u128;
    for entry in &payload.encounter_histogram {
        let zone_registry = zone_registry_for_entry(zone_registries, entry.zone_id)?;
        let enemy_archetype_registry = enemy_archetype_registry_for_entry(
            enemy_archetype_registries,
            entry.enemy_archetype_id,
        )?;
        require!(
            zone_registry.exp_multiplier_den > 0,
            SettlementError::InvalidZoneConfig
        );

        let weighted_exp = u128::from(entry.count)
            .checked_mul(u128::from(enemy_archetype_registry.exp_reward_base))
            .and_then(|value| value.checked_mul(u128::from(zone_registry.exp_multiplier_num)))
            .ok_or_else(|| error!(SettlementError::ArithmeticOverflow))?
            / u128::from(zone_registry.exp_multiplier_den);

        total_exp = total_exp
            .checked_add(weighted_exp)
            .ok_or_else(|| error!(SettlementError::ArithmeticOverflow))?;
    }

    u32::try_from(total_exp).map_err(|_| error!(SettlementError::ArithmeticOverflow))
}

fn apply_zone_progress_delta(
    payload: &SettlementBatchPayloadV1,
    primary_zone_progress_page: &mut CharacterZoneProgressPageAccount,
    additional_zone_progress_pages: &mut [LoadedZoneProgressPage],
    character_world_progress: &mut CharacterWorldProgressAccount,
) -> Result<()> {
    for entry in &payload.zone_progress_delta {
        let zone_offset = (entry.zone_id % ZONE_PAGE_WIDTH) as usize;
        let page_index = entry.zone_id / ZONE_PAGE_WIDTH;
        if primary_zone_progress_page.page_index == page_index {
            let prior_state = primary_zone_progress_page.zone_states[zone_offset];
            if entry.new_state > prior_state {
                primary_zone_progress_page.zone_states[zone_offset] = entry.new_state;
            }
        } else {
            let zone_page = additional_zone_progress_pages
                .iter_mut()
                .find(|page| page.data.page_index == page_index)
                .ok_or_else(|| error!(SettlementError::MissingZoneProgressPageAccount))?;
            let prior_state = zone_page.data.zone_states[zone_offset];
            if entry.new_state > prior_state {
                zone_page.data.zone_states[zone_offset] = entry.new_state;
            }
        }

        if entry.new_state >= ZONE_STATE_UNLOCKED {
            character_world_progress.highest_unlocked_zone_id = character_world_progress
                .highest_unlocked_zone_id
                .max(entry.zone_id);
        }
        if entry.new_state >= ZONE_STATE_CLEARED {
            character_world_progress.highest_cleared_zone_id = character_world_progress
                .highest_cleared_zone_id
                .max(entry.zone_id);
        }
    }

    Ok(())
}

fn persist_additional_zone_progress_pages<'info>(
    additional_zone_progress_pages: Vec<LoadedZoneProgressPage<'info>>,
    program_id: &Pubkey,
) -> Result<()> {
    for page in additional_zone_progress_pages {
        if page.info.owner == program_id && !anchor_lang::__private::is_closed(&page.info) {
            let mut data = page.info.try_borrow_mut_data()?;
            let dst: &mut [u8] = &mut data;
            let mut writer = anchor_lang::__private::BpfWriter::new(dst);
            page.data.try_serialize(&mut writer)?;
        }
    }

    Ok(())
}

fn canonical_server_attestation_message(
    program_id: &Pubkey,
    cluster_id: u8,
    character_root_pubkey: Pubkey,
    payload: &SettlementBatchPayloadV1,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(program_id.as_ref());
    out.push(cluster_id);
    out.extend_from_slice(character_root_pubkey.as_ref());
    out.extend_from_slice(&payload.character_id);
    out.extend_from_slice(&payload.batch_id.to_le_bytes());
    out.extend_from_slice(&payload.start_nonce.to_le_bytes());
    out.extend_from_slice(&payload.end_nonce.to_le_bytes());
    out.extend_from_slice(&payload.battle_count.to_le_bytes());
    out.extend_from_slice(&payload.first_battle_ts.to_le_bytes());
    out.extend_from_slice(&payload.last_battle_ts.to_le_bytes());
    out.extend_from_slice(&payload.season_id.to_le_bytes());
    out.extend_from_slice(&payload.start_state_hash);
    out.extend_from_slice(&payload.end_state_hash);
    put_zone_progress_delta_vec(&mut out, &payload.zone_progress_delta);
    put_encounter_histogram_vec(&mut out, &payload.encounter_histogram);
    put_option_u32(&mut out, payload.optional_loadout_revision);
    out.extend_from_slice(&payload.batch_hash);
    out.extend_from_slice(&payload.schema_version.to_le_bytes());
    out.push(payload.signature_scheme);
    out
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX_LOWER[(byte >> 4) as usize] as char);
        out.push(HEX_LOWER[(byte & 0x0f) as usize] as char);
    }
    out
}

fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    let mut index = 0;

    while index + 3 <= bytes.len() {
        let chunk = ((bytes[index] as u32) << 16)
            | ((bytes[index + 1] as u32) << 8)
            | (bytes[index + 2] as u32);
        out.push(ALPHABET[((chunk >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((chunk >> 12) & 0x3f) as usize] as char);
        out.push(ALPHABET[((chunk >> 6) & 0x3f) as usize] as char);
        out.push(ALPHABET[(chunk & 0x3f) as usize] as char);
        index += 3;
    }

    let remaining = bytes.len() - index;
    if remaining == 1 {
        let chunk = (bytes[index] as u32) << 16;
        out.push(ALPHABET[((chunk >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((chunk >> 12) & 0x3f) as usize] as char);
    } else if remaining == 2 {
        let chunk = ((bytes[index] as u32) << 16) | ((bytes[index + 1] as u32) << 8);
        out.push(ALPHABET[((chunk >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((chunk >> 12) & 0x3f) as usize] as char);
        out.push(ALPHABET[((chunk >> 6) & 0x3f) as usize] as char);
    }

    out
}

fn canonical_player_authorization_message_text(
    program_id: &Pubkey,
    cluster_id: u8,
    player_authority_pubkey: Pubkey,
    character_root_pubkey: Pubkey,
    batch_hash: [u8; 32],
    batch_id: u64,
    signature_scheme: u8,
) -> String {
    [
        "RUNANA".to_string(),
        "settlement".to_string(),
        signature_scheme.to_string(),
        cluster_id.to_string(),
        program_id.to_string(),
        player_authority_pubkey.to_string(),
        character_root_pubkey.to_string(),
        batch_id.to_string(),
        base64url(&batch_hash),
    ]
    .join("|")
}

fn canonical_player_authorization_message(
    program_id: &Pubkey,
    cluster_id: u8,
    player_authority_pubkey: Pubkey,
    character_root_pubkey: Pubkey,
    batch_hash: [u8; 32],
    batch_id: u64,
    signature_scheme: u8,
) -> Result<Vec<u8>> {
    match signature_scheme {
        SIGNATURE_SCHEME_ED25519_RAW_V1 => {
            let mut out = Vec::new();
            out.extend_from_slice(program_id.as_ref());
            out.push(cluster_id);
            out.extend_from_slice(player_authority_pubkey.as_ref());
            out.extend_from_slice(character_root_pubkey.as_ref());
            out.extend_from_slice(&batch_hash);
            out.extend_from_slice(&batch_id.to_le_bytes());
            out.push(signature_scheme);
            Ok(out)
        }
        SIGNATURE_SCHEME_WALLET_TEXT_V1 => Ok(canonical_player_authorization_message_text(
            program_id,
            cluster_id,
            player_authority_pubkey,
            character_root_pubkey,
            batch_hash,
            batch_id,
            signature_scheme,
        )
        .into_bytes()),
        _ => err!(SettlementError::UnsupportedSignatureScheme),
    }
}

fn put_zone_progress_delta_vec(out: &mut Vec<u8>, entries: &[ZoneProgressDeltaEntry]) {
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for entry in entries {
        out.extend_from_slice(&entry.zone_id.to_le_bytes());
        out.push(entry.new_state);
    }
}

fn put_encounter_histogram_vec(out: &mut Vec<u8>, entries: &[EncounterCountEntry]) {
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for entry in entries {
        out.extend_from_slice(&entry.zone_id.to_le_bytes());
        out.extend_from_slice(&entry.enemy_archetype_id.to_le_bytes());
        out.extend_from_slice(&entry.count.to_le_bytes());
    }
}

fn put_option_u32(out: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(inner) => {
            out.push(1);
            out.extend_from_slice(&inner.to_le_bytes());
        }
        None => out.push(0),
    }
}

#[error_code]
pub enum SettlementError {
    #[msg("The provided admin signer is not authorized for this registry mutation")]
    UnauthorizedAdmin,
    #[msg("Settlement is paused by program policy")]
    SettlementPaused,
    #[msg("The instructions sysvar account could not be parsed")]
    InvalidInstructionsSysvar,
    #[msg("Two ed25519 verification instructions must precede the settlement instruction")]
    MissingEd25519Preinstructions,
    #[msg("The settlement instruction must be preceded by two ed25519 instructions in order")]
    InvalidEd25519InstructionOrder,
    #[msg("The ed25519 verification instruction data does not match the canonical shape")]
    InvalidEd25519InstructionData,
    #[msg("The trusted server attestation contents do not match the settlement payload")]
    ServerAttestationMismatch,
    #[msg("The player authorization contents do not match the settlement payload")]
    PlayerAuthorizationMismatch,
    #[msg("The settlement payload uses an unsupported signature scheme")]
    UnsupportedSignatureScheme,
    #[msg("The settlement nonce range does not match battle_count")]
    InvalidNonceRange,
    #[msg("The encounter histogram total does not match battle_count")]
    HistogramCountMismatch,
    #[msg("The settlement batch exceeds the configured max_battles_per_batch")]
    BatchBattleCountLimitExceeded,
    #[msg("The settlement batch exceeds the configured max_histogram_entries_per_batch")]
    HistogramEntryLimitExceeded,
    #[msg("Duplicate encounter histogram zone/enemy pairs are forbidden")]
    DuplicateEncounterHistogramEntry,
    #[msg("Encounter histogram entries must have non-zero counts")]
    ZeroEncounterHistogramEntry,
    #[msg("The canonical settlement preimage could not be serialized")]
    PreimageSerializationFailed,
    #[msg("The recomputed batch hash does not match the payload batch hash")]
    BatchHashMismatch,
    #[msg("The provided character id does not match the character root account")]
    CharacterIdMismatch,
    #[msg("The settlement permit subject does not match the character authority")]
    PlayerAuthorityMismatch,
    #[msg("A derived character-side account is not bound to the provided character root")]
    CharacterAccountBindingMismatch,
    #[msg("The settlement start nonce must follow the cursor end nonce")]
    InvalidStartNonce,
    #[msg("The settlement batch id must be strictly sequential")]
    InvalidBatchSequence,
    #[msg("The payload start_state_hash does not match the cursor")]
    StartStateHashMismatch,
    #[msg("Player-owned account creation must be funded by the player authority")]
    PlayerMustSelfFund,
    #[msg("The program config PDA does not match the canonical seed")]
    InvalidProgramConfigPda,
    #[msg("The character root PDA does not match the canonical seed")]
    InvalidCharacterPda,
    #[msg("The character stats PDA does not match the canonical seed")]
    InvalidCharacterStatsPda,
    #[msg("The character world progress PDA does not match the canonical seed")]
    InvalidCharacterWorldProgressPda,
    #[msg("The character settlement cursor PDA does not match the canonical seed")]
    InvalidCharacterCursorPda,
    #[msg("The zone registry PDA does not match the canonical seed")]
    InvalidZoneRegistryPda,
    #[msg("The zone enemy set PDA does not match the canonical seed")]
    InvalidZoneEnemySetPda,
    #[msg("The enemy archetype PDA does not match the canonical seed")]
    InvalidEnemyArchetypePda,
    #[msg("The season policy PDA does not match the canonical seed")]
    InvalidSeasonPolicyPda,
    #[msg("The character zone progress page PDA does not match the canonical seed")]
    InvalidZoneProgressPagePda,
    #[msg("Zone progression updates require the matching page account")]
    InvalidZoneProgressPage,
    #[msg("The settlement batch is missing a required zone progress page account")]
    MissingZoneProgressPageAccount,
    #[msg("Zone progress page accounts must be supplied in canonical ascending page order")]
    ZoneProgressPageAccountOrderMismatch,
    #[msg("Zone progress page accounts used by settlement must be writable")]
    ZoneProgressPageMustBeWritable,
    #[msg("Zone enemy sets must contain a sorted unique list of legal enemy ids within the configured cap")]
    InvalidZoneEnemySet,
    #[msg("Zone progress delta entries violate the canonical monotonic transition rules")]
    InvalidZoneProgressDelta,
    #[msg("Duplicate zone progress delta entries are forbidden")]
    DuplicateZoneProgressDelta,
    #[msg("Summary and zone page progression state are inconsistent")]
    SummaryPageInconsistency,
    #[msg("Zone progress page state contains an invalid value")]
    InvalidZoneProgressState,
    #[msg("The zone configuration is invalid")]
    InvalidZoneConfig,
    #[msg("The season policy configuration is invalid")]
    InvalidSeasonPolicy,
    #[msg("The provided season policy does not match the settlement payload season id")]
    SeasonPolicyMismatch,
    #[msg("The provided zone enemy set is inconsistent with the zone or enemy registry")]
    ZoneEnemySetMismatch,
    #[msg("The settlement batch is missing a required zone registry account")]
    MissingZoneRegistryAccount,
    #[msg("The settlement batch is missing a required zone enemy set account")]
    MissingZoneEnemySetAccount,
    #[msg("The settlement batch is missing a required enemy archetype registry account")]
    MissingEnemyArchetypeRegistryAccount,
    #[msg("Settlement remaining accounts must be supplied in canonical grouped ascending order")]
    SettlementRemainingAccountOrderMismatch,
    #[msg(
        "Settlement received unexpected extra remaining accounts beyond the canonical grouped set"
    )]
    UnexpectedSettlementRemainingAccounts,
    #[msg("The settlement batch references a zone that is not unlocked for this character")]
    IllegalLockedZoneReference,
    #[msg("The settlement batch references an enemy that is not legal for the zone")]
    IllegalZoneEnemyPair,
    #[msg("The encounter histogram zone does not match the provided zone registry")]
    EncounterZoneMismatch,
    #[msg("The encounter histogram enemy does not match the provided enemy registry")]
    EncounterEnemyMismatch,
    #[msg("The first battle timestamp predates character creation")]
    PreCharacterTimestamp,
    #[msg("Battle timestamps must be monotonic and non-regressing")]
    BattleTimestampRegression,
    #[msg("The settlement season id must be monotonic")]
    SeasonRegression,
    #[msg("The settlement season window or grace window is closed")]
    SeasonWindowClosed,
    #[msg("The claimed battle density exceeds the deterministic throughput cap")]
    ThroughputExceeded,
    #[msg("The cluster clock timestamp is invalid")]
    InvalidClockTimestamp,
    #[msg("Settlement math overflowed")]
    ArithmeticOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seq_pubkey(start: u8) -> Pubkey {
        Pubkey::new_from_array(std::array::from_fn(|index| start.wrapping_add(index as u8)))
    }

    #[test]
    fn canonical_player_authorization_message_supports_legacy_raw_scheme() {
        let program_id = seq_pubkey(0x70);
        let player_authority = seq_pubkey(0xb0);
        let character_root = seq_pubkey(0x90);
        let batch_hash = std::array::from_fn(|index| 0x50_u8.wrapping_add(index as u8));

        let message = canonical_player_authorization_message(
            &program_id,
            CLUSTER_ID_LOCALNET,
            player_authority,
            character_root,
            batch_hash,
            7,
            SIGNATURE_SCHEME_ED25519_RAW_V1,
        )
        .unwrap();

        assert_eq!(lower_hex(&message), "707172737475767778797a7b7c7d7e7f808182838485868788898a8b8c8d8e8f01b0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0c1c2c3c4c5c6c7c8c9cacbcccdcecf909192939495969798999a9b9c9d9e9fa0a1a2a3a4a5a6a7a8a9aaabacadaeaf505152535455565758595a5b5c5d5e5f606162636465666768696a6b6c6d6e6f070000000000000000");
    }

    #[test]
    fn canonical_player_authorization_message_supports_wallet_text_scheme() {
        let program_id = seq_pubkey(0x70);
        let player_authority = seq_pubkey(0xb0);
        let character_root = seq_pubkey(0x90);
        let batch_hash = std::array::from_fn(|index| 0x50_u8.wrapping_add(index as u8));

        let message = canonical_player_authorization_message(
            &program_id,
            CLUSTER_ID_LOCALNET,
            player_authority,
            character_root,
            batch_hash,
            7,
            SIGNATURE_SCHEME_WALLET_TEXT_V1,
        )
        .unwrap();

        assert_eq!(
            String::from_utf8(message).unwrap(),
            format!(
                "RUNANA|settlement|1|{}|{}|{}|{}|7|UFFSU1RVVldYWVpbXF1eX2BhYmNkZWZnaGlqa2xtbm8",
                CLUSTER_ID_LOCALNET, program_id, player_authority, character_root,
            ),
        );
    }

    #[test]
    fn canonical_player_authorization_message_rejects_unknown_scheme() {
        let program_id = seq_pubkey(0x70);
        let player_authority = seq_pubkey(0xb0);
        let character_root = seq_pubkey(0x90);
        let batch_hash = std::array::from_fn(|index| 0x50_u8.wrapping_add(index as u8));

        let error = canonical_player_authorization_message(
            &program_id,
            CLUSTER_ID_LOCALNET,
            player_authority,
            character_root,
            batch_hash,
            7,
            9,
        )
        .unwrap_err();

        match error {
            anchor_lang::error::Error::AnchorError(anchor_error) => {
                assert_eq!(
                    anchor_error.error_code_number,
                    <SettlementError as Into<u32>>::into(
                        SettlementError::UnsupportedSignatureScheme,
                    ),
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
