use crate::{
    fixtures::canonical_authority_keypair,
    fixtures::unique_integration_fixture_set,
    integration_helpers::{build_dual_ed25519_verification_instructions, LocalnetRelayerHarness},
};
use anchor_client::solana_sdk::signature::Signer;
use std::time::{SystemTime, UNIX_EPOCH};

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_secs()
}

#[test]
fn test_create_character_requires_player_as_payer() {
    let fixtures = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_static_fixture_state(&fixtures)
        .expect("static fixture state should bootstrap");

    let tx = harness
        .submit_create_character_with_player_payer(&fixtures)
        .expect("player-funded character creation should succeed");

    harness
        .assert_signature_confirmed(&tx)
        .expect("player-funded character creation should be confirmed");

    let bad_fixtures = unique_integration_fixture_set();
    harness
        .bootstrap_slice1_static_fixture_state(&bad_fixtures)
        .expect("static fixture state should bootstrap for mismatched payer test");

    let err = harness
        .submit_create_character_with_mismatched_payer(&bad_fixtures)
        .expect_err("non-player-funded character creation should fail");

    assert!(
        err.to_string()
            .contains("Player-owned account creation must be funded by the player authority"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_happy_path() {
    let fixtures = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("slice 1 fixture state should bootstrap");
    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let instructions = harness
        .build_settlement_request_instructions(&fixtures, &pre_instructions)
        .expect("settlement request should build");

    assert_eq!(instructions.len(), 3);

    let tx = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect("happy-path settlement transaction should succeed");

    harness
        .assert_signature_confirmed(&tx)
        .expect("happy-path settlement transaction should be confirmed");

    let character_root = harness
        .fetch_anchor_account::<runana_program::CharacterRootAccount>(
            fixtures.character.character_root_pubkey,
        )
        .expect("character root fetch should succeed")
        .expect("character root should exist after bootstrap");
    let character_stats = harness
        .fetch_anchor_account::<runana_program::CharacterStatsAccount>(
            fixtures.character.character_stats_pubkey,
        )
        .expect("character stats fetch should succeed")
        .expect("character stats should exist after bootstrap");
    let character_world_progress = harness
        .fetch_anchor_account::<runana_program::CharacterWorldProgressAccount>(
            fixtures.character.character_world_progress_pubkey,
        )
        .expect("character world progress fetch should succeed")
        .expect("character world progress should exist after bootstrap");
    let character_zone_progress_page = harness
        .fetch_anchor_account::<runana_program::CharacterZoneProgressPageAccount>(
            fixtures.character.character_zone_progress_page_pubkey,
        )
        .expect("character zone progress fetch should succeed")
        .expect("character zone progress page should exist after bootstrap");
    let cursor = harness
        .fetch_anchor_account::<runana_program::CharacterSettlementBatchCursorAccount>(
            fixtures.character.character_settlement_batch_cursor_pubkey,
        )
        .expect("cursor fetch should succeed")
        .expect("cursor should exist after settlement");
    let season_policy = harness
        .fetch_anchor_account::<runana_program::SeasonPolicyAccount>(
            fixtures.season.season_policy_pubkey,
        )
        .expect("season policy fetch should succeed")
        .expect("season policy should exist after bootstrap");

    assert_eq!(character_root.authority, fixtures.character.authority);
    assert_eq!(character_root.character_id, fixtures.character.character_id);
    assert_eq!(character_stats.level, 1);
    assert_eq!(
        character_stats.total_exp,
        u64::from(fixtures.batch.derived_exp_delta)
    );
    assert_eq!(
        character_world_progress.highest_unlocked_zone_id,
        fixtures.zone.zone_id
    );
    assert_eq!(
        character_world_progress.highest_cleared_zone_id, 0,
        "happy-path fixture does not clear a zone in Slice 1"
    );
    assert_eq!(
        character_zone_progress_page.zone_states[fixtures.zone.zone_id as usize],
        1,
    );
    assert_eq!(
        cursor.last_committed_end_nonce,
        fixtures.batch.payload.end_nonce
    );
    assert_eq!(
        cursor.last_committed_batch_id,
        fixtures.batch.payload.batch_id
    );
    assert_eq!(
        cursor.last_committed_state_hash,
        fixtures.batch.payload.end_state_hash
    );
    assert_eq!(
        cursor.last_committed_battle_ts,
        fixtures.batch.payload.last_battle_ts
    );
    assert_eq!(
        cursor.last_committed_season_id,
        fixtures.batch.payload.season_id
    );
    assert_eq!(season_policy.season_id, fixtures.season.season_id);
}

#[test]
fn test_create_character_persists_historical_character_creation_timestamp() {
    let fixtures = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_static_fixture_state(&fixtures)
        .expect("static fixture state should bootstrap");

    let tx = harness
        .submit_create_character_with_player_payer(&fixtures)
        .expect("player-funded character creation should succeed");

    harness
        .assert_signature_confirmed(&tx)
        .expect("character creation transaction should be confirmed");

    let character_root = harness
        .fetch_anchor_account::<runana_program::CharacterRootAccount>(
            fixtures.character.character_root_pubkey,
        )
        .expect("character root fetch should succeed")
        .expect("character root should exist after creation");
    let cursor = harness
        .fetch_anchor_account::<runana_program::CharacterSettlementBatchCursorAccount>(
            fixtures.character.character_settlement_batch_cursor_pubkey,
        )
        .expect("cursor fetch should succeed")
        .expect("cursor should exist after creation");
    let season_policy = harness
        .fetch_anchor_account::<runana_program::SeasonPolicyAccount>(
            fixtures.season.season_policy_pubkey,
        )
        .expect("season policy fetch should succeed")
        .expect("season policy should exist after creation");

    assert!(
        season_policy.season_start_ts <= character_root.character_creation_ts
            && character_root.character_creation_ts <= season_policy.season_end_ts
    );
    assert_eq!(
        cursor.last_committed_battle_ts,
        character_root.character_creation_ts
    );
    assert_eq!(cursor.last_committed_season_id, season_policy.season_id);
}

#[test]
fn test_create_character_rejects_closed_season_window() {
    let now = current_unix_timestamp();
    let mut fixtures = unique_integration_fixture_set();
    fixtures.season.season_start_ts = now.saturating_add(3_600);
    fixtures.season.season_end_ts = now.saturating_add(7_200);
    fixtures.season.commit_grace_end_ts = now.saturating_add(10_800);
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_static_fixture_state(&fixtures)
        .expect("static fixture state should bootstrap");

    let err = harness
        .submit_create_character_with_player_payer(&fixtures)
        .expect_err("creation outside active season window should fail");

    assert!(
        err.to_string()
            .contains("The settlement season window or grace window is closed"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_accepts_create_and_settle_in_same_transaction() {
    let fixtures = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_static_fixture_state(&fixtures)
        .expect("static fixture state should bootstrap");

    let authority = canonical_authority_keypair();
    let create_instructions = harness
        .build_create_character_instructions(&fixtures, authority.pubkey(), authority.pubkey())
        .expect("create character instructions should build");
    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let settlement_instructions = harness
        .build_settlement_request_instructions(&fixtures, &[])
        .expect("settlement request should build");

    let instructions = create_instructions
        .into_iter()
        .chain(pre_instructions)
        .chain(settlement_instructions)
        .collect::<Vec<_>>();

    let tx = harness
        .submit_versioned_transaction_with_signers(&instructions, &authority, &[&authority])
        .expect("atomic create-plus-settle transaction should succeed");

    harness
        .assert_signature_confirmed(&tx)
        .expect("atomic create-plus-settle transaction should be confirmed");

    let character_root = harness
        .fetch_anchor_account::<runana_program::CharacterRootAccount>(
            fixtures.character.character_root_pubkey,
        )
        .expect("character root fetch should succeed")
        .expect("character root should exist after atomic sync");
    let cursor = harness
        .fetch_anchor_account::<runana_program::CharacterSettlementBatchCursorAccount>(
            fixtures.character.character_settlement_batch_cursor_pubkey,
        )
        .expect("cursor fetch should succeed")
        .expect("cursor should exist after atomic sync");

    assert!(
        fixtures.season.season_start_ts <= character_root.character_creation_ts
            && character_root.character_creation_ts <= fixtures.season.season_end_ts
    );
    assert_eq!(
        cursor.last_committed_end_nonce,
        fixtures.batch.payload.end_nonce
    );
    assert_eq!(
        cursor.last_committed_batch_id,
        fixtures.batch.payload.batch_id
    );
    assert_eq!(
        cursor.last_committed_battle_ts,
        fixtures.batch.payload.last_battle_ts
    );
}
