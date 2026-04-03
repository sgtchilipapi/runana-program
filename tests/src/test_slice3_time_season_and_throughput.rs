use std::time::{SystemTime, UNIX_EPOCH};

use anchor_client::solana_sdk::{hash::hashv, pubkey::Pubkey};

use crate::{
    fixtures::{
        canonical_batch_hash_preimage, canonical_player_authorization_message,
        canonical_server_attestation_message, unique_integration_fixture_set,
        CanonicalBatchFixture, CanonicalBatchPayloadFixture, CanonicalFixtureSet,
        EncounterCountEntryFixture, SEASON_POLICY_SEED,
    },
    integration_helpers::{build_dual_ed25519_verification_instructions, LocalnetRelayerHarness},
};

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_secs()
}

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

    let mut next = fixtures.clone();
    next.batch = CanonicalBatchFixture {
        payload,
        batch_hash,
        batch_hash_preimage,
        server_attestation_message,
        player_authorization_message,
        derived_exp_delta: fixtures.batch.derived_exp_delta,
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

fn with_season(
    fixtures: &CanonicalFixtureSet,
    season_id: u32,
    season_start_ts: u64,
    season_end_ts: u64,
    commit_grace_end_ts: u64,
) -> CanonicalFixtureSet {
    let mut next = fixtures.clone();
    let (season_policy_pubkey, _) = Pubkey::find_program_address(
        &[SEASON_POLICY_SEED, &season_id.to_le_bytes()],
        &fixtures.program.program_id,
    );
    next.season.season_id = season_id;
    next.season.season_policy_pubkey = season_policy_pubkey;
    next.season.season_start_ts = season_start_ts;
    next.season.season_end_ts = season_end_ts;
    next.season.commit_grace_end_ts = commit_grace_end_ts;
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
fn test_apply_battle_settlement_batch_v1_accepts_delayed_submission_within_grace() {
    let base = unique_integration_fixture_set();
    let now = current_unix_timestamp();
    let fixtures = with_season(
        &base,
        base.batch.payload.season_id,
        base.character.character_creation_ts.saturating_sub(60),
        base.batch.payload.last_battle_ts,
        now + 3_600,
    );
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let tx = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect("delayed submission within grace should succeed");

    harness
        .assert_signature_confirmed(&tx)
        .expect("delayed submission transaction should confirm");
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_grace_expired_submission() {
    let base = unique_integration_fixture_set();
    let now = current_unix_timestamp();
    let fixtures = with_season(
        &base,
        base.batch.payload.season_id,
        base.character.character_creation_ts.saturating_sub(60),
        base.batch.payload.last_battle_ts,
        now.saturating_sub(3_600),
    );
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("grace-expired submission should fail");

    assert_err_contains(
        err,
        "The settlement season window or grace window is closed",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_season_regression() {
    let base = unique_integration_fixture_set();
    let regressed_season_id = base.character.season_id_at_creation - 1;
    let fixtures = with_payload(
        &with_season(
            &base,
            regressed_season_id,
            base.character.character_creation_ts.saturating_sub(60),
            base.batch.payload.last_battle_ts + 60,
            current_unix_timestamp() + 3_600,
        ),
        |payload| {
            payload.season_id = regressed_season_id;
        },
    );
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("season regression should fail");

    assert_err_contains(err, "The settlement season id must be monotonic");
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_pre_character_timestamp() {
    let base = unique_integration_fixture_set();
    let fixtures = with_payload(&base, |payload| {
        payload.first_battle_ts = base.character.character_creation_ts - 1;
        payload.last_battle_ts = base.character.character_creation_ts;
    });
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("pre-character timestamp should fail");

    assert_err_contains(
        err,
        "The first battle timestamp predates character creation",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_accepts_throughput_boundary() {
    let base = unique_integration_fixture_set();
    let fixtures = with_payload(
        &with_season(
            &base,
            base.batch.payload.season_id,
            base.character.character_creation_ts.saturating_sub(60),
            base.character.character_creation_ts + 300,
            current_unix_timestamp() + 3_600,
        ),
        |payload| {
            payload.start_nonce = 1;
            payload.end_nonce = 21;
            payload.battle_count = 21;
            payload.first_battle_ts = base.character.character_creation_ts + 60;
            payload.last_battle_ts = payload.first_battle_ts + 60;
            payload.end_state_hash = hashv(&[b"slice3_throughput_pass"]).to_bytes();
            payload.encounter_histogram = vec![EncounterCountEntryFixture {
                zone_id: base.zone.zone_id,
                enemy_archetype_id: base.enemy.enemy_archetype_id,
                count: 21,
            }];
        },
    );
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let tx = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect("throughput boundary should succeed");

    harness
        .assert_signature_confirmed(&tx)
        .expect("throughput boundary transaction should confirm");
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_throughput_overflow() {
    let base = unique_integration_fixture_set();
    let fixtures = with_payload(
        &with_season(
            &base,
            base.batch.payload.season_id,
            base.character.character_creation_ts.saturating_sub(60),
            base.character.character_creation_ts + 300,
            current_unix_timestamp() + 3_600,
        ),
        |payload| {
            payload.start_nonce = 1;
            payload.end_nonce = 22;
            payload.battle_count = 22;
            payload.first_battle_ts = base.character.character_creation_ts + 60;
            payload.last_battle_ts = payload.first_battle_ts + 60;
            payload.end_state_hash = hashv(&[b"slice3_throughput_fail"]).to_bytes();
            payload.encounter_histogram = vec![EncounterCountEntryFixture {
                zone_id: base.zone.zone_id,
                enemy_archetype_id: base.enemy.enemy_archetype_id,
                count: 22,
            }];
        },
    );
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("throughput overflow should fail");

    assert_err_contains(
        err,
        "The claimed battle density exceeds the deterministic throughput cap",
    );
}
