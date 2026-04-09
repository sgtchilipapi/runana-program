use anchor_client::solana_sdk::{
    hash::hashv,
    pubkey::Pubkey,
    signer::{
        keypair::{keypair_from_seed, Keypair},
        Signer,
    },
};
use runana_program::{
    ApplyBattleSettlementBatchV1Args, CreateCharacterArgs, EncounterCountEntry,
    InitializeCharacterZoneProgressPageArgs, InitializeEnemyArchetypeRegistryArgs,
    InitializeProgramConfigArgs, InitializeSeasonPolicyArgs, InitializeZoneEnemySetArgs,
    InitializeZoneRegistryArgs, SettlementBatchPayloadV1, ZoneProgressDeltaEntry,
};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub const CLUSTER_ID_LOCALNET: u8 = 1;
pub const SIGNATURE_SCHEME_ED25519_DUAL_SIG_V1: u8 = 0;
pub const SCHEMA_VERSION_CANONICAL_V2: u16 = 2;
pub const ZONE_STATE_UNLOCKED: u8 = 1;
pub const ZONE_STATE_CLEARED: u8 = 2;
pub const CHARACTER_ZONE_PROGRESS_SEED: &[u8] = b"character_zone_progress";
pub const CHARACTER_BATCH_CURSOR_SEED: &[u8] = b"character_batch_cursor";
pub const SEASON_POLICY_SEED: &[u8] = b"season_policy";
pub const CANONICAL_AUTHORITY_SEED: [u8; 32] = [7; 32];
pub const CANONICAL_ADMIN_SEED: [u8; 32] = [8; 32];
pub const CANONICAL_SERVER_SIGNER_SEED: [u8; 32] = [9; 32];
pub const CANONICAL_RELAYER_SEED: [u8; 32] = [10; 32];
pub const CANONICAL_ALT_AUTHORITY_SEED: [u8; 32] = [11; 32];

static UNIQUE_FIXTURE_DISCRIMINATOR: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalFixtureSet {
    pub program: CanonicalProgramFixture,
    pub character: CanonicalCharacterFixture,
    pub season: CanonicalSeasonFixture,
    pub zone: CanonicalZoneFixture,
    pub enemy: CanonicalEnemyFixture,
    pub batch: CanonicalBatchFixture,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalProgramFixture {
    pub program_id: Pubkey,
    pub cluster_id: u8,
    pub admin_authority: Pubkey,
    pub trusted_server_signer: Pubkey,
    pub relayer: Pubkey,
    pub program_config_pubkey: Pubkey,
    pub max_battles_per_batch: u16,
    pub max_histogram_entries_per_batch: u16,
    pub settlement_paused: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCharacterFixture {
    pub authority: Pubkey,
    pub character_id: [u8; 16],
    pub character_creation_ts: u64,
    pub season_id_at_creation: u32,
    pub character_root_pubkey: Pubkey,
    pub character_stats_pubkey: Pubkey,
    pub character_world_progress_pubkey: Pubkey,
    pub character_zone_progress_page_pubkey: Pubkey,
    pub character_settlement_batch_cursor_pubkey: Pubkey,
    pub unlocked_zone_ids: Vec<u16>,
    pub cursor: CanonicalCursorFixture,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalSeasonFixture {
    pub season_id: u32,
    pub season_policy_pubkey: Pubkey,
    pub season_start_ts: u64,
    pub season_end_ts: u64,
    pub commit_grace_end_ts: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCursorFixture {
    pub last_committed_end_nonce: u64,
    pub last_committed_state_hash: [u8; 32],
    pub last_committed_batch_id: u64,
    pub last_committed_battle_ts: u64,
    pub last_committed_season_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalZoneFixture {
    pub zone_id: u16,
    pub page_index_u16: u16,
    pub zone_registry_pubkey: Pubkey,
    pub zone_enemy_set_pubkey: Pubkey,
    pub allowed_enemy_archetype_ids: Vec<u16>,
    pub starting_state: u8,
    pub exp_multiplier_num: u16,
    pub exp_multiplier_den: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalEnemyFixture {
    pub enemy_archetype_id: u16,
    pub enemy_archetype_pubkey: Pubkey,
    pub exp_reward_base: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalBatchFixture {
    pub payload: CanonicalBatchPayloadFixture,
    pub batch_hash: [u8; 32],
    pub batch_hash_preimage: Vec<u8>,
    pub server_attestation_message: Vec<u8>,
    pub player_authorization_message: Vec<u8>,
    pub derived_exp_delta: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalBatchPayloadFixture {
    pub character_id: [u8; 16],
    pub batch_id: u64,
    pub start_nonce: u64,
    pub end_nonce: u64,
    pub battle_count: u16,
    pub first_battle_ts: u64,
    pub last_battle_ts: u64,
    pub season_id: u32,
    pub start_state_hash: [u8; 32],
    pub end_state_hash: [u8; 32],
    pub zone_progress_delta: Vec<ZoneProgressDeltaEntryFixture>,
    pub encounter_histogram: Vec<EncounterCountEntryFixture>,
    pub optional_loadout_revision: Option<u32>,
    pub schema_version: u16,
    pub signature_scheme: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZoneProgressDeltaEntryFixture {
    pub zone_id: u16,
    pub new_state: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncounterCountEntryFixture {
    pub zone_id: u16,
    pub enemy_archetype_id: u16,
    pub count: u16,
}

pub fn canonical_fixture_set() -> CanonicalFixtureSet {
    canonical_fixture_set_with_discriminator(0)
}

pub fn canonical_fixture_set_with_discriminator(discriminator: u64) -> CanonicalFixtureSet {
    let program_id = runana_program::id();
    let authority = canonical_authority_keypair().pubkey();
    let admin_authority = canonical_admin_keypair().pubkey();
    let trusted_server_signer = canonical_server_signer_keypair().pubkey();
    let relayer = canonical_relayer_keypair().pubkey();
    let mut character_id = *b"char_fixture_000";
    character_id[8..16].copy_from_slice(&discriminator.to_le_bytes());
    let character_creation_ts: u64 = 1_720_000_000;
    let season_id_at_creation = 1_u32.saturating_add((discriminator as u32) & 0x3fff_ffff);
    let unique_registry_offset = discriminator.saturating_sub(1) as u16;
    let zone_id: u16 = if discriminator == 0 {
        7
    } else {
        7_u16.saturating_add(unique_registry_offset % 120)
    };
    let enemy_archetype_id: u16 = if discriminator == 0 {
        42
    } else {
        1_042_u16.saturating_add(unique_registry_offset)
    };
    let page_index_u16: u16 = zone_id / 256;

    let (program_config_pubkey, _) =
        Pubkey::find_program_address(&[b"program_config"], &program_id);
    let (character_root_pubkey, _) = Pubkey::find_program_address(
        &[b"character", authority.as_ref(), &character_id],
        &program_id,
    );
    let (character_stats_pubkey, _) = Pubkey::find_program_address(
        &[b"character_stats", character_root_pubkey.as_ref()],
        &program_id,
    );
    let (character_world_progress_pubkey, _) = Pubkey::find_program_address(
        &[b"character_world_progress", character_root_pubkey.as_ref()],
        &program_id,
    );
    let (character_zone_progress_page_pubkey, _) = Pubkey::find_program_address(
        &[
            b"character_zone_progress",
            character_root_pubkey.as_ref(),
            &page_index_u16.to_le_bytes(),
        ],
        &program_id,
    );
    let (character_settlement_batch_cursor_pubkey, _) = Pubkey::find_program_address(
        &[CHARACTER_BATCH_CURSOR_SEED, character_root_pubkey.as_ref()],
        &program_id,
    );
    let (zone_registry_pubkey, _) =
        Pubkey::find_program_address(&[b"zone_registry", &zone_id.to_le_bytes()], &program_id);
    let (zone_enemy_set_pubkey, _) =
        Pubkey::find_program_address(&[b"zone_enemy_set", &zone_id.to_le_bytes()], &program_id);
    let (enemy_archetype_pubkey, _) = Pubkey::find_program_address(
        &[b"enemy_archetype", &enemy_archetype_id.to_le_bytes()],
        &program_id,
    );
    let (season_policy_pubkey, _) = Pubkey::find_program_address(
        &[SEASON_POLICY_SEED, &season_id_at_creation.to_le_bytes()],
        &program_id,
    );

    let last_committed_state_hash = genesis_state_hash(character_root_pubkey, character_id);
    let cursor = CanonicalCursorFixture {
        last_committed_end_nonce: 0,
        last_committed_state_hash,
        last_committed_batch_id: 0,
        last_committed_battle_ts: character_creation_ts.saturating_sub(60),
        last_committed_season_id: season_id_at_creation,
    };

    let character = CanonicalCharacterFixture {
        authority,
        character_id,
        character_creation_ts,
        season_id_at_creation,
        character_root_pubkey,
        character_stats_pubkey,
        character_world_progress_pubkey,
        character_zone_progress_page_pubkey,
        character_settlement_batch_cursor_pubkey,
        unlocked_zone_ids: vec![zone_id],
        cursor,
    };

    let season = CanonicalSeasonFixture {
        season_id: season_id_at_creation,
        season_policy_pubkey,
        season_start_ts: character_creation_ts.saturating_sub(60),
        season_end_ts: character_creation_ts + 86_400,
        commit_grace_end_ts: 4_100_000_000,
    };

    let zone = CanonicalZoneFixture {
        zone_id,
        page_index_u16,
        zone_registry_pubkey,
        zone_enemy_set_pubkey,
        allowed_enemy_archetype_ids: vec![enemy_archetype_id],
        starting_state: ZONE_STATE_UNLOCKED,
        exp_multiplier_num: 125,
        exp_multiplier_den: 100,
    };

    let enemy = CanonicalEnemyFixture {
        enemy_archetype_id,
        enemy_archetype_pubkey,
        exp_reward_base: 20,
    };

    let payload = CanonicalBatchPayloadFixture {
        character_id,
        batch_id: 1,
        start_nonce: 1,
        end_nonce: 3,
        battle_count: 3,
        first_battle_ts: character_creation_ts + 60,
        last_battle_ts: character_creation_ts + 180,
        season_id: season_id_at_creation,
        start_state_hash: character.cursor.last_committed_state_hash,
        end_state_hash: fixture_end_state_hash(
            character.character_root_pubkey,
            character_id,
            1,
            3,
            character_creation_ts + 180,
        ),
        zone_progress_delta: Vec::new(),
        encounter_histogram: vec![EncounterCountEntryFixture {
            zone_id,
            enemy_archetype_id,
            count: 3,
        }],
        optional_loadout_revision: None,
        schema_version: SCHEMA_VERSION_CANONICAL_V2,
        signature_scheme: SIGNATURE_SCHEME_ED25519_DUAL_SIG_V1,
    };

    let batch_hash_preimage = canonical_batch_hash_preimage(&payload);
    let batch_hash = hashv(&[&batch_hash_preimage]).to_bytes();
    let server_attestation_message = canonical_server_attestation_message(
        program_id,
        CLUSTER_ID_LOCALNET,
        character.character_root_pubkey,
        &payload,
        batch_hash,
    );
    let player_authorization_message = canonical_player_authorization_message(
        program_id,
        CLUSTER_ID_LOCALNET,
        character.authority,
        character.character_root_pubkey,
        batch_hash,
        payload.batch_id,
        payload.signature_scheme,
    );
    let derived_exp_delta = derive_exp_delta(&payload.encounter_histogram, &zone, &enemy);

    let program = CanonicalProgramFixture {
        program_id,
        cluster_id: CLUSTER_ID_LOCALNET,
        admin_authority,
        trusted_server_signer,
        relayer,
        program_config_pubkey,
        max_battles_per_batch: 32,
        max_histogram_entries_per_batch: 64,
        settlement_paused: false,
    };

    let batch = CanonicalBatchFixture {
        payload,
        batch_hash,
        batch_hash_preimage,
        server_attestation_message,
        player_authorization_message,
        derived_exp_delta,
    };

    CanonicalFixtureSet {
        program,
        character,
        season,
        zone,
        enemy,
        batch,
    }
}

pub fn unique_integration_fixture_set() -> CanonicalFixtureSet {
    let discriminator = UNIQUE_FIXTURE_DISCRIMINATOR.fetch_add(1, Ordering::Relaxed);
    rebuild_fixture_timestamps(
        canonical_fixture_set_with_discriminator(discriminator),
        current_unix_timestamp().saturating_add(30),
    )
}

pub fn genesis_state_hash(character_root_pubkey: Pubkey, character_id: [u8; 16]) -> [u8; 32] {
    hashv(&[
        character_root_pubkey.as_ref(),
        &character_id,
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
    ])
    .to_bytes()
}

pub fn canonical_batch_hash_preimage(payload: &CanonicalBatchPayloadFixture) -> Vec<u8> {
    let mut out = Vec::new();
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
    out.extend_from_slice(&payload.schema_version.to_le_bytes());
    out.push(payload.signature_scheme);
    out
}

pub fn canonical_server_attestation_message(
    program_id: Pubkey,
    cluster_id: u8,
    character_root_pubkey: Pubkey,
    payload: &CanonicalBatchPayloadFixture,
    batch_hash: [u8; 32],
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
    out.extend_from_slice(&batch_hash);
    out.extend_from_slice(&payload.schema_version.to_le_bytes());
    out.push(payload.signature_scheme);
    out
}

pub fn canonical_player_authorization_message(
    program_id: Pubkey,
    cluster_id: u8,
    player_authority_pubkey: Pubkey,
    character_root_pubkey: Pubkey,
    batch_hash: [u8; 32],
    batch_id: u64,
    signature_scheme: u8,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(program_id.as_ref());
    out.push(cluster_id);
    out.extend_from_slice(player_authority_pubkey.as_ref());
    out.extend_from_slice(character_root_pubkey.as_ref());
    out.extend_from_slice(&batch_hash);
    out.extend_from_slice(&batch_id.to_le_bytes());
    out.push(signature_scheme);
    out
}

pub fn derive_exp_delta(
    encounter_histogram: &[EncounterCountEntryFixture],
    zone: &CanonicalZoneFixture,
    enemy: &CanonicalEnemyFixture,
) -> u32 {
    let mut total_exp_u128 = 0_u128;
    for entry in encounter_histogram {
        let weighted_exp_u128 = u128::from(entry.count)
            * u128::from(enemy.exp_reward_base)
            * u128::from(zone.exp_multiplier_num)
            / u128::from(zone.exp_multiplier_den);
        total_exp_u128 += weighted_exp_u128;
    }
    total_exp_u128 as u32
}

fn fixture_end_state_hash(
    character_root_pubkey: Pubkey,
    character_id: [u8; 16],
    batch_id: u64,
    end_nonce: u64,
    last_battle_ts: u64,
) -> [u8; 32] {
    hashv(&[
        b"runana_fixture_end_state_v1",
        character_root_pubkey.as_ref(),
        &character_id,
        &batch_id.to_le_bytes(),
        &end_nonce.to_le_bytes(),
        &last_battle_ts.to_le_bytes(),
    ])
    .to_bytes()
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_secs()
}

fn rebuild_fixture_timestamps(
    mut fixtures: CanonicalFixtureSet,
    character_creation_ts: u64,
) -> CanonicalFixtureSet {
    fixtures.character.character_creation_ts = character_creation_ts;
    fixtures.season.season_start_ts = character_creation_ts.saturating_sub(60);
    fixtures.character.cursor.last_committed_battle_ts = fixtures.season.season_start_ts;
    fixtures.season.season_end_ts = character_creation_ts.saturating_add(86_400);
    fixtures.batch.payload.first_battle_ts = character_creation_ts.saturating_add(60);
    fixtures.batch.payload.last_battle_ts = character_creation_ts.saturating_add(180);
    fixtures.batch.payload.end_state_hash = fixture_end_state_hash(
        fixtures.character.character_root_pubkey,
        fixtures.character.character_id,
        fixtures.batch.payload.batch_id,
        fixtures.batch.payload.end_nonce,
        fixtures.batch.payload.last_battle_ts,
    );

    let batch_hash_preimage = canonical_batch_hash_preimage(&fixtures.batch.payload);
    let batch_hash = hashv(&[&batch_hash_preimage]).to_bytes();
    fixtures.batch.batch_hash_preimage = batch_hash_preimage;
    fixtures.batch.batch_hash = batch_hash;
    fixtures.batch.server_attestation_message = canonical_server_attestation_message(
        fixtures.program.program_id,
        fixtures.program.cluster_id,
        fixtures.character.character_root_pubkey,
        &fixtures.batch.payload,
        batch_hash,
    );
    fixtures.batch.player_authorization_message = canonical_player_authorization_message(
        fixtures.program.program_id,
        fixtures.program.cluster_id,
        fixtures.character.authority,
        fixtures.character.character_root_pubkey,
        batch_hash,
        fixtures.batch.payload.batch_id,
        fixtures.batch.payload.signature_scheme,
    );

    fixtures
}

fn put_zone_progress_delta_vec(out: &mut Vec<u8>, entries: &[ZoneProgressDeltaEntryFixture]) {
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for entry in entries {
        out.extend_from_slice(&entry.zone_id.to_le_bytes());
        out.push(entry.new_state);
    }
}

fn put_encounter_histogram_vec(out: &mut Vec<u8>, entries: &[EncounterCountEntryFixture]) {
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

pub fn canonical_authority_keypair() -> Keypair {
    keypair_from_seed(&CANONICAL_AUTHORITY_SEED).expect("authority seed should be valid")
}

pub fn canonical_admin_keypair() -> Keypair {
    keypair_from_seed(&CANONICAL_ADMIN_SEED).expect("admin seed should be valid")
}

pub fn canonical_server_signer_keypair() -> Keypair {
    keypair_from_seed(&CANONICAL_SERVER_SIGNER_SEED).expect("server signer seed should be valid")
}

pub fn canonical_relayer_keypair() -> Keypair {
    keypair_from_seed(&CANONICAL_RELAYER_SEED).expect("relayer seed should be valid")
}

pub fn canonical_alt_authority_keypair() -> Keypair {
    keypair_from_seed(&CANONICAL_ALT_AUTHORITY_SEED).expect("alt authority seed should be valid")
}

pub fn canonical_apply_battle_settlement_batch_v1_args() -> ApplyBattleSettlementBatchV1Args {
    let fixtures = canonical_fixture_set();
    apply_battle_settlement_batch_v1_args_for_fixture(&fixtures)
}

pub fn apply_battle_settlement_batch_v1_args_for_fixture(
    fixtures: &CanonicalFixtureSet,
) -> ApplyBattleSettlementBatchV1Args {
    ApplyBattleSettlementBatchV1Args {
        payload: to_program_batch_payload(&fixtures.batch.payload, fixtures.batch.batch_hash),
    }
}

pub fn initialize_program_config_args_for_fixture(
    fixtures: &CanonicalFixtureSet,
) -> InitializeProgramConfigArgs {
    InitializeProgramConfigArgs {
        trusted_server_signer: fixtures.program.trusted_server_signer,
        settlement_paused: fixtures.program.settlement_paused,
        max_battles_per_batch: fixtures.program.max_battles_per_batch,
        max_histogram_entries_per_batch: fixtures.program.max_histogram_entries_per_batch,
    }
}

pub fn initialize_zone_registry_args_for_fixture(
    fixtures: &CanonicalFixtureSet,
) -> InitializeZoneRegistryArgs {
    InitializeZoneRegistryArgs {
        zone_id: fixtures.zone.zone_id,
        exp_multiplier_num: fixtures.zone.exp_multiplier_num,
        exp_multiplier_den: fixtures.zone.exp_multiplier_den,
    }
}

pub fn initialize_zone_enemy_set_args_for_fixture(
    fixtures: &CanonicalFixtureSet,
) -> InitializeZoneEnemySetArgs {
    InitializeZoneEnemySetArgs {
        zone_id: fixtures.zone.zone_id,
        allowed_enemy_archetype_ids: fixtures.zone.allowed_enemy_archetype_ids.clone(),
    }
}

pub fn initialize_enemy_archetype_registry_args_for_fixture(
    fixtures: &CanonicalFixtureSet,
) -> InitializeEnemyArchetypeRegistryArgs {
    InitializeEnemyArchetypeRegistryArgs {
        enemy_archetype_id: fixtures.enemy.enemy_archetype_id,
        exp_reward_base: fixtures.enemy.exp_reward_base,
    }
}

pub fn initialize_season_policy_args_for_fixture(
    fixtures: &CanonicalFixtureSet,
) -> InitializeSeasonPolicyArgs {
    InitializeSeasonPolicyArgs {
        season_id: fixtures.season.season_id,
        season_start_ts: fixtures.season.season_start_ts,
        season_end_ts: fixtures.season.season_end_ts,
        commit_grace_end_ts: fixtures.season.commit_grace_end_ts,
    }
}

pub fn create_character_args_for_fixture(fixtures: &CanonicalFixtureSet) -> CreateCharacterArgs {
    CreateCharacterArgs {
        character_id: fixtures.character.character_id,
        initial_unlocked_zone_id: fixtures.zone.zone_id,
    }
}

pub fn initialize_character_zone_progress_page_args(
    page_index: u16,
) -> InitializeCharacterZoneProgressPageArgs {
    InitializeCharacterZoneProgressPageArgs { page_index }
}

pub fn to_program_batch_payload(
    payload: &CanonicalBatchPayloadFixture,
    batch_hash: [u8; 32],
) -> SettlementBatchPayloadV1 {
    SettlementBatchPayloadV1 {
        character_id: payload.character_id,
        batch_id: payload.batch_id,
        start_nonce: payload.start_nonce,
        end_nonce: payload.end_nonce,
        battle_count: payload.battle_count,
        start_state_hash: payload.start_state_hash,
        end_state_hash: payload.end_state_hash,
        zone_progress_delta: payload
            .zone_progress_delta
            .iter()
            .map(|entry| ZoneProgressDeltaEntry {
                zone_id: entry.zone_id,
                new_state: entry.new_state,
            })
            .collect(),
        encounter_histogram: payload
            .encounter_histogram
            .iter()
            .map(|entry| EncounterCountEntry {
                zone_id: entry.zone_id,
                enemy_archetype_id: entry.enemy_archetype_id,
                count: entry.count,
            })
            .collect(),
        optional_loadout_revision: payload.optional_loadout_revision,
        batch_hash,
        first_battle_ts: payload.first_battle_ts,
        last_battle_ts: payload.last_battle_ts,
        season_id: payload.season_id,
        schema_version: payload.schema_version,
        signature_scheme: payload.signature_scheme,
    }
}
