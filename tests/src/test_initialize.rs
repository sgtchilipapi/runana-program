use crate::{
    fixtures::canonical_fixture_set,
    integration_helpers::{build_dual_ed25519_verification_instructions, LocalnetRelayerHarness},
};

#[test]
fn test_apply_battle_settlement_batch_v1_smoke() {
    let fixtures = canonical_fixture_set();
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let instructions = harness
        .build_settlement_request_instructions(&pre_instructions)
        .expect("settlement request should build");

    assert_eq!(instructions.len(), 3);

    let tx = harness
        .submit_settlement_with_pre_instructions(&pre_instructions)
        .expect("settlement smoke transaction should succeed");

    harness
        .assert_signature_confirmed(&tx)
        .expect("settlement smoke transaction should be confirmed");
    harness
        .assert_accounts_missing(&[
            fixtures.character.character_root_pubkey,
            fixtures.character.character_stats_pubkey,
            fixtures.character.character_world_progress_pubkey,
            fixtures.character.character_settlement_batch_cursor_pubkey,
        ])
        .expect("settlement smoke instruction should not create settlement fixture accounts yet");
}
