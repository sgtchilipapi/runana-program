use crate::fixtures::{
    canonical_apply_battle_settlement_batch_v1_args, canonical_batch_hash_preimage,
    canonical_fixture_set, genesis_state_hash, CLUSTER_ID_LOCALNET, SCHEMA_VERSION_CANONICAL_V2,
    SIGNATURE_SCHEME_ED25519_DUAL_SIG_V1,
};

#[test]
fn canonical_fixture_set_is_self_consistent() {
    let fixtures = canonical_fixture_set();

    assert_eq!(fixtures.program.cluster_id, CLUSTER_ID_LOCALNET);
    assert_eq!(
        fixtures.batch.payload.schema_version,
        SCHEMA_VERSION_CANONICAL_V2
    );
    assert_eq!(
        fixtures.batch.payload.signature_scheme,
        SIGNATURE_SCHEME_ED25519_DUAL_SIG_V1
    );
    assert_eq!(fixtures.zone.page_index_u16, 0);
    assert_eq!(
        fixtures.character.unlocked_zone_ids,
        vec![fixtures.zone.zone_id]
    );
    assert_eq!(
        fixtures.character.cursor.last_committed_state_hash,
        genesis_state_hash(
            fixtures.character.character_root_pubkey,
            fixtures.character.character_id,
        )
    );
    assert_eq!(
        fixtures.batch.payload.start_state_hash,
        fixtures.character.cursor.last_committed_state_hash
    );
    assert_eq!(fixtures.batch.payload.batch_id, 1);
    assert_eq!(fixtures.batch.payload.start_nonce, 1);
    assert_eq!(fixtures.batch.payload.end_nonce, 3);
    assert_eq!(fixtures.batch.payload.battle_count, 3);
    assert_eq!(
        u64::from(fixtures.batch.payload.battle_count),
        fixtures.batch.payload.end_nonce - fixtures.batch.payload.start_nonce + 1
    );
    assert_eq!(
        fixtures.batch.payload.season_id,
        fixtures.character.season_id_at_creation
    );
    assert_eq!(fixtures.season.season_id, fixtures.batch.payload.season_id);
    assert!(
        fixtures.season.season_start_ts <= fixtures.batch.payload.first_battle_ts
            && fixtures.batch.payload.last_battle_ts <= fixtures.season.season_end_ts
    );
    assert!(fixtures.season.season_end_ts <= fixtures.season.commit_grace_end_ts);
    assert!(fixtures.batch.payload.zone_progress_delta.is_empty());
    assert_eq!(fixtures.batch.payload.encounter_histogram.len(), 1);
    assert_eq!(
        fixtures.batch.payload.encounter_histogram[0].zone_id,
        fixtures.zone.zone_id
    );
    assert_eq!(
        fixtures.batch.payload.encounter_histogram[0].enemy_archetype_id,
        fixtures.enemy.enemy_archetype_id
    );
    assert_eq!(fixtures.batch.payload.encounter_histogram[0].count, 3);
    assert_eq!(fixtures.batch.derived_exp_delta, 75);
}

#[test]
fn canonical_fixture_set_is_deterministic() {
    let first = canonical_fixture_set();
    let second = canonical_fixture_set();

    assert_eq!(first, second);
    assert_eq!(
        first.batch.batch_hash_preimage,
        canonical_batch_hash_preimage(&first.batch.payload)
    );
    assert_eq!(first.batch.batch_hash.len(), 32);
    assert!(!first.batch.batch_hash_preimage.is_empty());
    assert!(!first.batch.server_attestation_message.is_empty());
    assert!(!first.batch.player_authorization_message.is_empty());
}

#[test]
fn canonical_fixture_maps_to_program_instruction_args() {
    let fixtures = canonical_fixture_set();
    let args = canonical_apply_battle_settlement_batch_v1_args();

    assert_eq!(
        args.payload.character_id,
        fixtures.batch.payload.character_id
    );
    assert_eq!(args.payload.batch_id, fixtures.batch.payload.batch_id);
    assert_eq!(
        args.payload.battle_count,
        fixtures.batch.payload.battle_count
    );
    assert_eq!(args.payload.batch_hash, fixtures.batch.batch_hash);
    assert_eq!(
        args.payload.encounter_histogram.len(),
        fixtures.batch.payload.encounter_histogram.len()
    );
}
