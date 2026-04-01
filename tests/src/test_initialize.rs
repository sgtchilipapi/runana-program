use crate::{
    fixtures::canonical_fixture_set,
    integration_helpers::{build_dual_ed25519_verification_instructions, LocalnetRelayerHarness},
};

#[test]
fn test_initialize() {
    let fixtures = canonical_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let instructions = harness
        .build_initialize_request_instructions(&pre_instructions)
        .expect("initialize request should build");

    assert_eq!(instructions.len(), 3);

    let tx = harness
        .submit_initialize_with_pre_instructions(&pre_instructions)
        .expect("initialize transaction should succeed");

    harness
        .assert_signature_confirmed(&tx)
        .expect("initialize transaction should be confirmed");
    harness
        .assert_accounts_missing(&[
            fixtures.character.character_root_pubkey,
            fixtures.character.character_stats_pubkey,
            fixtures.character.character_world_progress_pubkey,
            fixtures.character.character_settlement_batch_cursor_pubkey,
        ])
        .expect("initialize should not create settlement fixture accounts");
}
