use anchor_client::solana_sdk::{hash::hashv, pubkey::Pubkey, signer::Signer};

use crate::{
    fixtures::{
        apply_battle_settlement_batch_v1_args_for_fixture, canonical_alt_authority_keypair,
        canonical_authority_keypair, canonical_batch_hash_preimage,
        canonical_player_authorization_message, canonical_server_attestation_message,
        canonical_server_signer_keypair, unique_integration_fixture_set, CanonicalBatchFixture,
        CanonicalBatchPayloadFixture, CanonicalFixtureSet,
    },
    integration_helpers::{
        build_dual_ed25519_verification_instructions, build_ed25519_verification_instruction,
        sign_arbitrary_message, sign_server_attestation, LocalnetRelayerHarness,
    },
};

fn with_payload(
    fixtures: &CanonicalFixtureSet,
    payload: CanonicalBatchPayloadFixture,
) -> CanonicalFixtureSet {
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

fn assert_err_contains(err: Box<dyn std::error::Error>, expected: &str) {
    let rendered = err.to_string();
    assert!(
        rendered.contains(expected),
        "expected error containing {expected:?}, got {rendered:?}",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_replayed_batch() {
    let fixtures = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let signature = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect("first settlement should succeed");
    harness
        .assert_signature_confirmed(&signature)
        .expect("first settlement should confirm");

    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("replayed settlement should fail");

    assert_err_contains(
        err,
        "The settlement start nonce must follow the cursor end nonce",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_out_of_order_batch_id() {
    let base = unique_integration_fixture_set();
    let mut payload = base.batch.payload.clone();
    payload.batch_id = 2;
    let fixtures = with_payload(&base, payload);
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("out-of-order batch id should fail");

    assert_err_contains(err, "The settlement batch id must be strictly sequential");
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_player_permit_with_wrong_batch_hash() {
    let fixtures = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let server_signed = sign_server_attestation(&fixtures);
    let wrong_batch_hash = hashv(&[b"wrong_batch_hash"]).to_bytes();
    let wrong_player_message = canonical_player_authorization_message(
        fixtures.program.program_id,
        fixtures.program.cluster_id,
        fixtures.character.authority,
        fixtures.character.character_root_pubkey,
        wrong_batch_hash,
        fixtures.batch.payload.batch_id,
        fixtures.batch.payload.signature_scheme,
    );
    let player_signed =
        sign_arbitrary_message(&canonical_authority_keypair(), &wrong_player_message);
    let pre_instructions = vec![
        build_ed25519_verification_instruction(&server_signed),
        build_ed25519_verification_instruction(&player_signed),
    ];

    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("wrong player permit batch hash should fail");

    assert_err_contains(
        err,
        "The player authorization contents do not match the settlement payload",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_player_permit_with_wrong_batch_id() {
    let fixtures = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let server_signed = sign_server_attestation(&fixtures);
    let wrong_player_message = canonical_player_authorization_message(
        fixtures.program.program_id,
        fixtures.program.cluster_id,
        fixtures.character.authority,
        fixtures.character.character_root_pubkey,
        fixtures.batch.batch_hash,
        fixtures.batch.payload.batch_id + 1,
        fixtures.batch.payload.signature_scheme,
    );
    let player_signed =
        sign_arbitrary_message(&canonical_authority_keypair(), &wrong_player_message);
    let pre_instructions = vec![
        build_ed25519_verification_instruction(&server_signed),
        build_ed25519_verification_instruction(&player_signed),
    ];

    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("wrong player permit batch id should fail");

    assert_err_contains(
        err,
        "The player authorization contents do not match the settlement payload",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_wrong_character_owner() {
    let fixtures = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let alt_authority = canonical_alt_authority_keypair();
    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let instructions = harness
        .build_settlement_request_instructions_with_accounts_and_args(
            &fixtures,
            alt_authority.pubkey(),
            apply_battle_settlement_batch_v1_args_for_fixture(&fixtures),
            &pre_instructions,
        )
        .expect("instruction build should succeed");
    let canonical_authority = canonical_authority_keypair();
    let err = harness
        .submit_versioned_transaction_with_signers(
            &instructions,
            &canonical_authority,
            &[&canonical_authority],
        )
        .expect_err("wrong character owner should fail");

    assert_err_contains(
        err,
        "The settlement permit subject does not match the character authority",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_server_signature_domain_mismatch() {
    let fixtures = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let wrong_server_message = canonical_server_attestation_message(
        Pubkey::new_unique(),
        fixtures.program.cluster_id,
        fixtures.character.character_root_pubkey,
        &fixtures.batch.payload,
        fixtures.batch.batch_hash,
    );
    let server_signed =
        sign_arbitrary_message(&canonical_server_signer_keypair(), &wrong_server_message);
    let player_signed = sign_arbitrary_message(
        &canonical_authority_keypair(),
        &fixtures.batch.player_authorization_message,
    );
    let pre_instructions = vec![
        build_ed25519_verification_instruction(&server_signed),
        build_ed25519_verification_instruction(&player_signed),
    ];

    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("wrong server signature domain should fail");

    assert_err_contains(
        err,
        "The trusted server attestation contents do not match the settlement payload",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_player_signature_domain_mismatch() {
    let fixtures = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let server_signed = sign_server_attestation(&fixtures);
    let wrong_player_message = canonical_player_authorization_message(
        fixtures.program.program_id,
        fixtures.program.cluster_id + 1,
        fixtures.character.authority,
        fixtures.character.character_root_pubkey,
        fixtures.batch.batch_hash,
        fixtures.batch.payload.batch_id,
        fixtures.batch.payload.signature_scheme,
    );
    let player_signed =
        sign_arbitrary_message(&canonical_authority_keypair(), &wrong_player_message);
    let pre_instructions = vec![
        build_ed25519_verification_instruction(&server_signed),
        build_ed25519_verification_instruction(&player_signed),
    ];

    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("wrong player signature domain should fail");

    assert_err_contains(
        err,
        "The player authorization contents do not match the settlement payload",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_reversed_ed25519_instruction_order() {
    let fixtures = unique_integration_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");

    let mut pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    pre_instructions.reverse();

    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("reversed ed25519 instruction order should fail");

    assert_err_contains(
        err,
        "The settlement instruction must be preceded by two ed25519 instructions in order",
    );
}
