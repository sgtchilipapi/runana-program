use anchor_client::solana_sdk::signer::Signer;

use crate::{
    fixtures::{
        canonical_authority_keypair, canonical_fixture_set, canonical_server_signer_keypair,
    },
    integration_helpers::{
        build_dual_ed25519_verification_instructions, build_ed25519_verification_instruction,
        sign_player_authorization, sign_server_attestation,
    },
};

#[test]
fn integration_helpers_sign_with_fixture_owned_keys() {
    let fixtures = canonical_fixture_set();

    let signed_server = sign_server_attestation(&fixtures);
    let signed_player = sign_player_authorization(&fixtures);

    assert_eq!(
        signed_server.signer_pubkey,
        canonical_server_signer_keypair().pubkey()
    );
    assert_eq!(
        signed_player.signer_pubkey,
        canonical_authority_keypair().pubkey()
    );
    assert_eq!(
        signed_server.signer_pubkey,
        fixtures.program.trusted_server_signer
    );
    assert_eq!(signed_player.signer_pubkey, fixtures.character.authority);
    assert_eq!(
        signed_server.message,
        fixtures.batch.server_attestation_message
    );
    assert_eq!(
        signed_player.message,
        fixtures.batch.player_authorization_message
    );
}

#[test]
fn integration_helpers_build_dual_ed25519_preinstructions() {
    let fixtures = canonical_fixture_set();

    let signed_server = sign_server_attestation(&fixtures);
    let server_ix = build_ed25519_verification_instruction(&signed_server);
    let dual_ixs = build_dual_ed25519_verification_instructions(&fixtures);

    assert_ne!(server_ix.program_id, runana_program::id());
    assert!(server_ix.accounts.is_empty());
    assert_eq!(dual_ixs.len(), 2);
    assert_eq!(dual_ixs[0].program_id, server_ix.program_id);
    assert_eq!(dual_ixs[1].program_id, server_ix.program_id);
    assert!(dual_ixs.iter().all(|ix| !ix.data.is_empty()));
}
