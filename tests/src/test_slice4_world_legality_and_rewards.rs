use std::time::{SystemTime, UNIX_EPOCH};

use anchor_client::solana_sdk::pubkey::Pubkey;

use crate::{
    fixtures::{
        canonical_batch_hash_preimage, canonical_player_authorization_message,
        canonical_server_attestation_message, derive_exp_delta, unique_integration_fixture_set,
        CanonicalBatchFixture, CanonicalBatchPayloadFixture, CanonicalFixtureSet,
        EncounterCountEntryFixture, ZoneProgressDeltaEntryFixture, ZONE_STATE_UNLOCKED,
    },
    integration_helpers::{build_dual_ed25519_verification_instructions, LocalnetRelayerHarness},
};

const CHARACTER_ZONE_PROGRESS_SEED: &[u8] = b"character_zone_progress";

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_secs()
}

fn rebuild_batch(fixtures: &CanonicalFixtureSet) -> CanonicalFixtureSet {
    let payload = fixtures.batch.payload.clone();
    let batch_hash_preimage = canonical_batch_hash_preimage(&payload);
    let batch_hash = anchor_client::solana_sdk::hash::hashv(&[&batch_hash_preimage]).to_bytes();
    let server_attestation_message = canonical_server_attestation_message(
        fixtures.program.program_id,
        fixtures.program.cluster_id,
        fixtures.character.character_root_pubkey,
        &payload,
        batch_hash,
    );
    let player_authorization_message = canonical_player_authorization_message(
        fixtures.program.program_id,
        fixtures.program.cluster_id,
        fixtures.character.authority,
        fixtures.character.character_root_pubkey,
        batch_hash,
        payload.batch_id,
        payload.signature_scheme,
    );
    let derived_exp_delta = derive_exp_delta(
        &payload.encounter_histogram,
        &fixtures.zone,
        &fixtures.enemy,
    );

    let mut next = fixtures.clone();
    next.batch = CanonicalBatchFixture {
        payload,
        batch_hash,
        batch_hash_preimage,
        server_attestation_message,
        player_authorization_message,
        derived_exp_delta,
    };
    next
}

fn with_payload(
    fixtures: &CanonicalFixtureSet,
    mutate: impl FnOnce(&mut CanonicalBatchPayloadFixture),
) -> CanonicalFixtureSet {
    let mut next = fixtures.clone();
    mutate(&mut next.batch.payload);
    rebuild_batch(&next)
}

fn with_registry_context(
    fixtures: &CanonicalFixtureSet,
    zone_id: u16,
    enemy_archetype_id: u16,
    exp_reward_base: u32,
    exp_multiplier_num: u16,
    exp_multiplier_den: u16,
) -> CanonicalFixtureSet {
    let mut next = fixtures.clone();
    let page_index_u16 = zone_id / 256;
    let program_id = fixtures.program.program_id;

    let (zone_registry_pubkey, _) =
        Pubkey::find_program_address(&[b"zone_registry", &zone_id.to_le_bytes()], &program_id);
    let (zone_enemy_set_pubkey, _) =
        Pubkey::find_program_address(&[b"zone_enemy_set", &zone_id.to_le_bytes()], &program_id);
    let (enemy_archetype_pubkey, _) = Pubkey::find_program_address(
        &[b"enemy_archetype", &enemy_archetype_id.to_le_bytes()],
        &program_id,
    );
    let (character_zone_progress_page_pubkey, _) = Pubkey::find_program_address(
        &[
            CHARACTER_ZONE_PROGRESS_SEED,
            fixtures.character.character_root_pubkey.as_ref(),
            &page_index_u16.to_le_bytes(),
        ],
        &program_id,
    );

    next.zone.zone_id = zone_id;
    next.zone.page_index_u16 = page_index_u16;
    next.zone.zone_registry_pubkey = zone_registry_pubkey;
    next.zone.zone_enemy_set_pubkey = zone_enemy_set_pubkey;
    next.zone.exp_multiplier_num = exp_multiplier_num;
    next.zone.exp_multiplier_den = exp_multiplier_den;

    next.enemy.enemy_archetype_id = enemy_archetype_id;
    next.enemy.enemy_archetype_pubkey = enemy_archetype_pubkey;
    next.enemy.exp_reward_base = exp_reward_base;

    next.character.character_zone_progress_page_pubkey = character_zone_progress_page_pubkey;

    next
}

fn assert_err_contains(err: Box<dyn std::error::Error>, expected: &str) {
    let rendered = err.to_string();
    assert!(
        rendered.contains(expected),
        "expected error containing {expected:?}, got {rendered:?}",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_illegal_locked_zone_reference() {
    let base = unique_integration_fixture_set();
    let zone_id = base.zone.zone_id + 1;
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&base)
        .expect("base fixture state should bootstrap");

    let fixtures = with_payload(
        &with_registry_context(
            &base,
            zone_id,
            base.enemy.enemy_archetype_id,
            base.enemy.exp_reward_base,
            base.zone.exp_multiplier_num,
            base.zone.exp_multiplier_den,
        ),
        |payload| {
            payload.encounter_histogram = vec![EncounterCountEntryFixture {
                zone_id,
                enemy_archetype_id: base.enemy.enemy_archetype_id,
                count: payload.battle_count,
            }];
        },
    );
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("alternate registry fixture state should bootstrap");

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("locked zone reference should fail");

    assert_err_contains(
        err,
        "The settlement batch references a zone that is not unlocked for this character",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_illegal_zone_enemy_pair() {
    let base = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&base)
        .expect("fixture state should bootstrap");

    let fixtures = with_payload(&base, |payload| {
        payload.encounter_histogram = vec![EncounterCountEntryFixture {
            zone_id: base.zone.zone_id,
            enemy_archetype_id: base.enemy.enemy_archetype_id + 1,
            count: payload.battle_count,
        }];
    });

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("illegal zone/enemy pair should fail");

    assert_err_contains(
        err,
        "The settlement batch references an enemy that is not legal for the zone",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_duplicate_histogram_entries() {
    let base = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&base)
        .expect("fixture state should bootstrap");

    let fixtures = with_payload(&base, |payload| {
        payload.encounter_histogram = vec![
            EncounterCountEntryFixture {
                zone_id: base.zone.zone_id,
                enemy_archetype_id: base.enemy.enemy_archetype_id,
                count: 1,
            },
            EncounterCountEntryFixture {
                zone_id: base.zone.zone_id,
                enemy_archetype_id: base.enemy.enemy_archetype_id,
                count: payload.battle_count - 1,
            },
        ];
    });

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("duplicate histogram entries should fail");

    assert_err_contains(
        err,
        "Duplicate encounter histogram zone/enemy pairs are forbidden",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_zero_count_histogram_entry() {
    let base = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&base)
        .expect("fixture state should bootstrap");

    let fixtures = with_payload(&base, |payload| {
        payload.end_nonce = payload.start_nonce;
        payload.battle_count = 1;
        payload.last_battle_ts = payload.first_battle_ts;
        payload.encounter_histogram = vec![EncounterCountEntryFixture {
            zone_id: base.zone.zone_id,
            enemy_archetype_id: base.enemy.enemy_archetype_id,
            count: 0,
        }];
    });

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("zero-count histogram entry should fail");

    assert_err_contains(err, "Encounter histogram entries must have non-zero counts");
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_exp_overflow() {
    let base = unique_integration_fixture_set();
    let zone_id = base.zone.zone_id + 3;
    let enemy_archetype_id = base.enemy.enemy_archetype_id + 1000;
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&base)
        .expect("base fixture state should bootstrap");

    let fixtures = with_payload(
        &with_registry_context(&base, zone_id, enemy_archetype_id, u32::MAX, u16::MAX, 1),
        |payload| {
            payload.end_nonce = 32;
            payload.battle_count = 32;
            payload.last_battle_ts = payload.first_battle_ts + 93;
            payload.zone_progress_delta = vec![ZoneProgressDeltaEntryFixture {
                zone_id,
                new_state: ZONE_STATE_UNLOCKED,
            }];
            payload.encounter_histogram = vec![EncounterCountEntryFixture {
                zone_id,
                enemy_archetype_id,
                count: 32,
            }];
        },
    );
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("overflow registry fixture state should bootstrap");

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("overflowing exp derivation should fail");

    assert_err_contains(err, "Settlement math overflowed");
}

#[test]
fn test_apply_battle_settlement_batch_v1_accepts_same_batch_zone_unlock_for_legal_reward() {
    let base = unique_integration_fixture_set();
    let zone_id = base.zone.zone_id + 5;
    let enemy_archetype_id = base.enemy.enemy_archetype_id + 200;
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&base)
        .expect("base fixture state should bootstrap");

    let fixtures = with_payload(
        &with_registry_context(&base, zone_id, enemy_archetype_id, 40, 150, 100),
        |payload| {
            payload.end_nonce = 2;
            payload.battle_count = 2;
            payload.last_battle_ts = payload.first_battle_ts + 60;
            payload.zone_progress_delta = vec![ZoneProgressDeltaEntryFixture {
                zone_id,
                new_state: ZONE_STATE_UNLOCKED,
            }];
            payload.encounter_histogram = vec![EncounterCountEntryFixture {
                zone_id,
                enemy_archetype_id,
                count: 2,
            }];
        },
    );
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("alternate legal registry fixture state should bootstrap");

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let tx = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect("same-batch unlock should allow a legal settlement");

    harness
        .assert_signature_confirmed(&tx)
        .expect("same-batch unlock transaction should confirm");

    let character_stats = harness
        .fetch_anchor_account::<runana_program::CharacterStatsAccount>(
            fixtures.character.character_stats_pubkey,
        )
        .expect("character stats fetch should succeed")
        .expect("character stats should exist after settlement");
    let zone_progress_page = harness
        .fetch_anchor_account::<runana_program::CharacterZoneProgressPageAccount>(
            fixtures.character.character_zone_progress_page_pubkey,
        )
        .expect("zone progress page fetch should succeed")
        .expect("zone progress page should exist after settlement");

    assert_eq!(character_stats.total_exp, 120);
    assert_eq!(
        zone_progress_page.zone_states[zone_id as usize],
        ZONE_STATE_UNLOCKED
    );
    assert!(current_unix_timestamp() <= fixtures.season.commit_grace_end_ts);
}
