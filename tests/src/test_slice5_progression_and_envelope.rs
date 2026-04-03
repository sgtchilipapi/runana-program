use anchor_client::solana_sdk::{hash::hashv, pubkey::Pubkey};

use crate::{
    fixtures::{
        canonical_authority_keypair, canonical_batch_hash_preimage,
        canonical_player_authorization_message, canonical_server_attestation_message,
        derive_exp_delta, unique_integration_fixture_set, CanonicalBatchFixture,
        CanonicalBatchPayloadFixture, CanonicalFixtureSet, EncounterCountEntryFixture,
        ZoneProgressDeltaEntryFixture, CHARACTER_ZONE_PROGRESS_SEED, ZONE_STATE_CLEARED,
        ZONE_STATE_UNLOCKED,
    },
    integration_helpers::{build_dual_ed25519_verification_instructions, LocalnetRelayerHarness},
};

fn rebuild_batch(fixtures: &CanonicalFixtureSet) -> CanonicalFixtureSet {
    let payload = fixtures.batch.payload.clone();
    let batch_hash_preimage = canonical_batch_hash_preimage(&payload);
    let batch_hash = hashv(&[&batch_hash_preimage]).to_bytes();
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
    let program_id = fixtures.program.program_id;

    let (zone_registry_pubkey, _) =
        Pubkey::find_program_address(&[b"zone_registry", &zone_id.to_le_bytes()], &program_id);
    let (zone_enemy_set_pubkey, _) =
        Pubkey::find_program_address(&[b"zone_enemy_set", &zone_id.to_le_bytes()], &program_id);
    let (enemy_archetype_pubkey, _) = Pubkey::find_program_address(
        &[b"enemy_archetype", &enemy_archetype_id.to_le_bytes()],
        &program_id,
    );

    next.zone.zone_id = zone_id;
    next.zone.page_index_u16 = zone_id / 256;
    next.zone.zone_registry_pubkey = zone_registry_pubkey;
    next.zone.zone_enemy_set_pubkey = zone_enemy_set_pubkey;
    next.zone.allowed_enemy_archetype_ids = vec![enemy_archetype_id];
    next.zone.exp_multiplier_num = exp_multiplier_num;
    next.zone.exp_multiplier_den = exp_multiplier_den;

    next.enemy.enemy_archetype_id = enemy_archetype_id;
    next.enemy.enemy_archetype_pubkey = enemy_archetype_pubkey;
    next.enemy.exp_reward_base = exp_reward_base;

    next
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

fn zone_progress_page_pubkey(character_root_pubkey: Pubkey, page_index: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[
            CHARACTER_ZONE_PROGRESS_SEED,
            character_root_pubkey.as_ref(),
            &page_index.to_le_bytes(),
        ],
        &runana_program::id(),
    )
    .0
}

fn assert_err_contains(err: Box<dyn std::error::Error>, expected: &str) {
    let rendered = err.to_string();
    assert!(
        rendered.contains(expected),
        "expected error containing {expected:?}, got {rendered:?}",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_locked_to_cleared_transition() {
    let base = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&base)
        .expect("fixture state should bootstrap");

    let fixtures = with_payload(&base, |payload| {
        payload.zone_progress_delta = vec![ZoneProgressDeltaEntryFixture {
            zone_id: base.zone.zone_id + 1,
            new_state: ZONE_STATE_CLEARED,
        }];
    });

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("locked to cleared transition should fail");

    assert_err_contains(
        err,
        "Zone progress delta entries violate the canonical monotonic transition rules",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_missing_secondary_zone_progress_page() {
    let base = unique_integration_fixture_set();
    let zone_id = 260_u16;
    let enemy_archetype_id = base.enemy.enemy_archetype_id + 300;
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&base)
        .expect("base fixture state should bootstrap");

    let fixtures = with_payload(
        &with_registry_context(&base, zone_id, enemy_archetype_id, 35, 100, 100),
        |payload| {
            payload.zone_progress_delta = vec![
                ZoneProgressDeltaEntryFixture {
                    zone_id: base.zone.zone_id,
                    new_state: ZONE_STATE_CLEARED,
                },
                ZoneProgressDeltaEntryFixture {
                    zone_id,
                    new_state: ZONE_STATE_UNLOCKED,
                },
            ];
            payload.encounter_histogram = vec![EncounterCountEntryFixture {
                zone_id,
                enemy_archetype_id,
                count: payload.battle_count,
            }];
        },
    );
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("alternate registry fixture state should bootstrap");

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let mut instructions = harness
        .build_settlement_request_instructions(&fixtures, &pre_instructions)
        .expect("settlement instructions should build");
    let settlement_ix = instructions
        .last_mut()
        .expect("settlement instruction should be present");
    settlement_ix.accounts.truncate(9);

    let authority = canonical_authority_keypair();
    let err = harness
        .submit_versioned_transaction_with_signers(&instructions, &authority, &[&authority])
        .expect_err("missing secondary page account should fail");

    assert_err_contains(
        err,
        "The settlement batch is missing a required zone progress page account",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_supports_sequential_multi_page_progression() {
    let base = unique_integration_fixture_set();
    let zone_id = 516_u16;
    let enemy_archetype_id = base.enemy.enemy_archetype_id + 400;
    let second_page_index = zone_id / 256;
    let second_page_pubkey =
        zone_progress_page_pubkey(base.character.character_root_pubkey, second_page_index);
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&base)
        .expect("base fixture state should bootstrap");

    let authority = canonical_authority_keypair();
    harness
        .ensure_character_zone_progress_page(
            base.character.character_root_pubkey,
            &authority,
            second_page_index,
        )
        .expect("secondary zone progress page should initialize");

    let batch_one = base.clone();
    let batch_one_pre_instructions = build_dual_ed25519_verification_instructions(&batch_one);
    let tx = harness
        .submit_settlement_with_pre_instructions(&batch_one, &batch_one_pre_instructions)
        .expect("first sequential batch should succeed");
    harness
        .assert_signature_confirmed(&tx)
        .expect("first sequential batch should confirm");

    let batch_two = with_payload(
        &with_registry_context(&base, zone_id, enemy_archetype_id, 30, 100, 100),
        |payload| {
            payload.batch_id = 2;
            payload.start_nonce = batch_one.batch.payload.end_nonce + 1;
            payload.end_nonce = payload.start_nonce + 2;
            payload.battle_count = 3;
            payload.first_battle_ts = batch_one.batch.payload.last_battle_ts + 60;
            payload.last_battle_ts = payload.first_battle_ts + 120;
            payload.start_state_hash = batch_one.batch.payload.end_state_hash;
            payload.end_state_hash = fixture_end_state_hash(
                base.character.character_root_pubkey,
                base.character.character_id,
                payload.batch_id,
                payload.end_nonce,
                payload.last_battle_ts,
            );
            payload.zone_progress_delta = vec![
                ZoneProgressDeltaEntryFixture {
                    zone_id: base.zone.zone_id,
                    new_state: ZONE_STATE_CLEARED,
                },
                ZoneProgressDeltaEntryFixture {
                    zone_id,
                    new_state: ZONE_STATE_UNLOCKED,
                },
            ];
            payload.encounter_histogram = vec![EncounterCountEntryFixture {
                zone_id,
                enemy_archetype_id,
                count: payload.battle_count,
            }];
        },
    );
    harness
        .bootstrap_slice1_fixture_state(&batch_two)
        .expect("second batch registry fixture state should bootstrap");

    let batch_two_pre_instructions = build_dual_ed25519_verification_instructions(&batch_two);
    let tx = harness
        .submit_settlement_with_pre_instructions_and_extra_pages(
            &batch_two,
            &batch_two_pre_instructions,
            &[second_page_pubkey],
        )
        .expect("second sequential batch with multi-page progression should succeed");
    harness
        .assert_signature_confirmed(&tx)
        .expect("second sequential batch should confirm");

    let character_stats = harness
        .fetch_anchor_account::<runana_program::CharacterStatsAccount>(
            batch_two.character.character_stats_pubkey,
        )
        .expect("character stats fetch should succeed")
        .expect("character stats should exist after settlements");
    let character_world_progress = harness
        .fetch_anchor_account::<runana_program::CharacterWorldProgressAccount>(
            batch_two.character.character_world_progress_pubkey,
        )
        .expect("character world progress fetch should succeed")
        .expect("character world progress should exist after settlements");
    let primary_page = harness
        .fetch_anchor_account::<runana_program::CharacterZoneProgressPageAccount>(
            batch_two.character.character_zone_progress_page_pubkey,
        )
        .expect("primary page fetch should succeed")
        .expect("primary page should exist after settlements");
    let secondary_page = harness
        .fetch_anchor_account::<runana_program::CharacterZoneProgressPageAccount>(
            second_page_pubkey,
        )
        .expect("secondary page fetch should succeed")
        .expect("secondary page should exist after settlements");
    let cursor = harness
        .fetch_anchor_account::<runana_program::CharacterSettlementBatchCursorAccount>(
            batch_two.character.character_settlement_batch_cursor_pubkey,
        )
        .expect("cursor fetch should succeed")
        .expect("cursor should exist after settlements");

    assert_eq!(
        character_stats.total_exp,
        u64::from(batch_one.batch.derived_exp_delta + batch_two.batch.derived_exp_delta)
    );
    assert_eq!(
        primary_page.zone_states[base.zone.zone_id as usize],
        ZONE_STATE_CLEARED
    );
    assert_eq!(
        secondary_page.zone_states[(zone_id % 256) as usize],
        ZONE_STATE_UNLOCKED
    );
    assert_eq!(character_world_progress.highest_unlocked_zone_id, zone_id);
    assert_eq!(
        character_world_progress.highest_cleared_zone_id,
        base.zone.zone_id
    );
    assert_eq!(
        cursor.last_committed_batch_id,
        batch_two.batch.payload.batch_id
    );
    assert_eq!(
        cursor.last_committed_end_nonce,
        batch_two.batch.payload.end_nonce
    );
    assert_eq!(
        cursor.last_committed_state_hash,
        batch_two.batch.payload.end_state_hash
    );
    assert_eq!(
        cursor.last_committed_battle_ts,
        batch_two.batch.payload.last_battle_ts
    );
}
