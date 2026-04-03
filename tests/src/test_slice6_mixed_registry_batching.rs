use anchor_client::solana_sdk::{hash::hashv, pubkey::Pubkey};

use crate::{
    fixtures::{
        canonical_authority_keypair, canonical_batch_hash_preimage,
        canonical_player_authorization_message, canonical_server_attestation_message,
        unique_integration_fixture_set, CanonicalBatchFixture, CanonicalBatchPayloadFixture,
        CanonicalFixtureSet, EncounterCountEntryFixture, ZoneProgressDeltaEntryFixture,
        ZONE_STATE_UNLOCKED,
    },
    integration_helpers::{build_dual_ed25519_verification_instructions, LocalnetRelayerHarness},
};

fn rebuild_batch(fixtures: &CanonicalFixtureSet, derived_exp_delta: u32) -> CanonicalFixtureSet {
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
        derived_exp_delta,
    };
    next
}

fn with_payload(
    fixtures: &CanonicalFixtureSet,
    derived_exp_delta: u32,
    mutate: impl FnOnce(&mut CanonicalBatchPayloadFixture),
) -> CanonicalFixtureSet {
    let mut next = fixtures.clone();
    mutate(&mut next.batch.payload);
    rebuild_batch(&next, derived_exp_delta)
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

fn expected_exp(
    payload: &CanonicalBatchPayloadFixture,
    zones: &[(u16, u16, u16)],
    enemies: &[(u16, u32)],
) -> u32 {
    let mut total = 0_u128;

    for entry in &payload.encounter_histogram {
        let zone = zones
            .iter()
            .find(|(zone_id, _, _)| *zone_id == entry.zone_id)
            .expect("zone registry should exist");
        let enemy = enemies
            .iter()
            .find(|(enemy_id, _)| *enemy_id == entry.enemy_archetype_id)
            .expect("enemy registry should exist");
        total +=
            u128::from(entry.count) * u128::from(enemy.1) * u128::from(zone.1) / u128::from(zone.2);
    }

    total as u32
}

fn ensure_zone_bundle(
    harness: &LocalnetRelayerHarness,
    fixtures: &CanonicalFixtureSet,
    zone_id: u16,
    exp_multiplier_num: u16,
    exp_multiplier_den: u16,
    allowed_enemy_archetype_ids: Vec<u16>,
    enemies: &[(u16, u32)],
) {
    harness
        .ensure_zone_registry_entry(
            fixtures.program.program_config_pubkey,
            zone_id,
            exp_multiplier_num,
            exp_multiplier_den,
        )
        .expect("zone registry should initialize");
    harness
        .upsert_zone_enemy_set_entry(
            fixtures.program.program_config_pubkey,
            zone_id,
            allowed_enemy_archetype_ids,
        )
        .expect("zone enemy set should initialize");
    for (enemy_archetype_id, exp_reward_base) in enemies {
        harness
            .ensure_enemy_archetype_registry_entry(
                fixtures.program.program_config_pubkey,
                *enemy_archetype_id,
                *exp_reward_base,
            )
            .expect("enemy archetype registry should initialize");
    }
}

fn settlement_instruction_mut(
    instructions: &mut [anchor_client::solana_sdk::instruction::Instruction],
) -> &mut anchor_client::solana_sdk::instruction::Instruction {
    instructions
        .last_mut()
        .expect("settlement instruction should be present")
}

fn zone_registry_pubkey(zone_id: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[b"zone_registry", &zone_id.to_le_bytes()],
        &runana_program::id(),
    )
    .0
}

fn zone_enemy_set_pubkey(zone_id: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[b"zone_enemy_set", &zone_id.to_le_bytes()],
        &runana_program::id(),
    )
    .0
}

fn enemy_archetype_pubkey(enemy_archetype_id: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[b"enemy_archetype", &enemy_archetype_id.to_le_bytes()],
        &runana_program::id(),
    )
    .0
}

fn find_account_index(
    instruction: &anchor_client::solana_sdk::instruction::Instruction,
    pubkey: Pubkey,
) -> usize {
    instruction
        .accounts
        .iter()
        .position(|meta| meta.pubkey == pubkey)
        .expect("expected account should be present")
}

fn assert_err_contains(err: Box<dyn std::error::Error>, expected: &str) {
    let rendered = err.to_string();
    assert!(
        rendered.contains(expected),
        "expected error containing {expected:?}, got {rendered:?}",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_accepts_mixed_zones_and_enemies() {
    let mut fixtures = unique_integration_fixture_set();
    let second_enemy_id = fixtures.enemy.enemy_archetype_id + 101;
    let second_zone_id = fixtures.zone.zone_id + 21;
    let third_enemy_id = fixtures.enemy.enemy_archetype_id + 102;
    fixtures.zone.allowed_enemy_archetype_ids =
        vec![fixtures.enemy.enemy_archetype_id, second_enemy_id];

    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");
    harness
        .ensure_enemy_archetype_registry_entry(
            fixtures.program.program_config_pubkey,
            second_enemy_id,
            32,
        )
        .expect("second enemy registry should initialize");
    ensure_zone_bundle(
        &harness,
        &fixtures,
        second_zone_id,
        150,
        100,
        vec![third_enemy_id],
        &[(third_enemy_id, 10)],
    );

    let expected = expected_exp(
        &CanonicalBatchPayloadFixture {
            encounter_histogram: vec![
                EncounterCountEntryFixture {
                    zone_id: fixtures.zone.zone_id,
                    enemy_archetype_id: fixtures.enemy.enemy_archetype_id,
                    count: 1,
                },
                EncounterCountEntryFixture {
                    zone_id: fixtures.zone.zone_id,
                    enemy_archetype_id: second_enemy_id,
                    count: 1,
                },
                EncounterCountEntryFixture {
                    zone_id: second_zone_id,
                    enemy_archetype_id: third_enemy_id,
                    count: 2,
                },
            ],
            ..fixtures.batch.payload.clone()
        },
        &[
            (
                fixtures.zone.zone_id,
                fixtures.zone.exp_multiplier_num,
                fixtures.zone.exp_multiplier_den,
            ),
            (second_zone_id, 150, 100),
        ],
        &[
            (
                fixtures.enemy.enemy_archetype_id,
                fixtures.enemy.exp_reward_base,
            ),
            (second_enemy_id, 32),
            (third_enemy_id, 10),
        ],
    );
    let fixtures = with_payload(&fixtures, expected, |payload| {
        payload.end_nonce = payload.start_nonce + 3;
        payload.battle_count = 4;
        payload.last_battle_ts = payload.first_battle_ts + 180;
        payload.zone_progress_delta = vec![ZoneProgressDeltaEntryFixture {
            zone_id: second_zone_id,
            new_state: ZONE_STATE_UNLOCKED,
        }];
        payload.encounter_histogram = vec![
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: fixtures.enemy.enemy_archetype_id,
                count: 1,
            },
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: second_enemy_id,
                count: 1,
            },
            EncounterCountEntryFixture {
                zone_id: second_zone_id,
                enemy_archetype_id: third_enemy_id,
                count: 2,
            },
        ];
    });

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let tx = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect("mixed settlement should succeed");
    harness
        .assert_signature_confirmed(&tx)
        .expect("mixed settlement should confirm");

    let character_stats = harness
        .fetch_anchor_account::<runana_program::CharacterStatsAccount>(
            fixtures.character.character_stats_pubkey,
        )
        .expect("character stats fetch should succeed")
        .expect("character stats should exist");
    let zone_progress_page = harness
        .fetch_anchor_account::<runana_program::CharacterZoneProgressPageAccount>(
            fixtures.character.character_zone_progress_page_pubkey,
        )
        .expect("zone progress fetch should succeed")
        .expect("zone progress page should exist");

    assert_eq!(character_stats.total_exp, 95);
    assert_eq!(
        zone_progress_page.zone_states[second_zone_id as usize],
        ZONE_STATE_UNLOCKED
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_accepts_multiple_legal_enemies_in_one_zone() {
    let mut fixtures = unique_integration_fixture_set();
    let second_enemy_id = fixtures.enemy.enemy_archetype_id + 201;
    fixtures.zone.allowed_enemy_archetype_ids =
        vec![fixtures.enemy.enemy_archetype_id, second_enemy_id];

    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");
    harness
        .ensure_enemy_archetype_registry_entry(
            fixtures.program.program_config_pubkey,
            second_enemy_id,
            24,
        )
        .expect("second enemy registry should initialize");

    let fixtures = with_payload(&fixtures, 85, |payload| {
        payload.encounter_histogram = vec![
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: fixtures.enemy.enemy_archetype_id,
                count: 1,
            },
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: second_enemy_id,
                count: 2,
            },
        ];
    });

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let tx = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect("same-zone mixed enemy settlement should succeed");
    harness
        .assert_signature_confirmed(&tx)
        .expect("same-zone mixed enemy settlement should confirm");

    let character_stats = harness
        .fetch_anchor_account::<runana_program::CharacterStatsAccount>(
            fixtures.character.character_stats_pubkey,
        )
        .expect("character stats fetch should succeed")
        .expect("character stats should exist");

    assert_eq!(character_stats.total_exp, 85);
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_missing_zone_registry_account() {
    let mut fixtures = unique_integration_fixture_set();
    let second_zone_id = fixtures.zone.zone_id + 31;
    let second_enemy_id = fixtures.enemy.enemy_archetype_id + 302;
    fixtures.zone.allowed_enemy_archetype_ids = vec![fixtures.enemy.enemy_archetype_id];

    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");
    ensure_zone_bundle(
        &harness,
        &fixtures,
        second_zone_id,
        100,
        100,
        vec![second_enemy_id],
        &[(second_enemy_id, 15)],
    );

    let fixtures = with_payload(&fixtures, 55, |payload| {
        payload.end_nonce = payload.start_nonce + 3;
        payload.battle_count = 4;
        payload.zone_progress_delta = vec![ZoneProgressDeltaEntryFixture {
            zone_id: second_zone_id,
            new_state: ZONE_STATE_UNLOCKED,
        }];
        payload.encounter_histogram = vec![
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: fixtures.enemy.enemy_archetype_id,
                count: 1,
            },
            EncounterCountEntryFixture {
                zone_id: second_zone_id,
                enemy_archetype_id: second_enemy_id,
                count: 3,
            },
        ];
    });

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let mut instructions = harness
        .build_settlement_request_instructions(&fixtures, &pre_instructions)
        .expect("instructions should build");
    let settlement_ix = settlement_instruction_mut(&mut instructions);
    let index = find_account_index(settlement_ix, zone_registry_pubkey(second_zone_id));
    settlement_ix.accounts.remove(index);

    let authority = canonical_authority_keypair();
    let err = harness
        .submit_versioned_transaction_with_signers(&instructions, &authority, &[&authority])
        .expect_err("missing zone registry should fail");

    assert_err_contains(
        err,
        "The settlement batch is missing a required zone registry account",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_missing_zone_enemy_set_account() {
    let fixtures = unique_integration_fixture_set();
    let second_zone_id = fixtures.zone.zone_id + 41;
    let second_enemy_id = fixtures.enemy.enemy_archetype_id + 402;
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");
    ensure_zone_bundle(
        &harness,
        &fixtures,
        second_zone_id,
        100,
        100,
        vec![second_enemy_id],
        &[(second_enemy_id, 15)],
    );

    let fixtures = with_payload(&fixtures, 55, |payload| {
        payload.end_nonce = payload.start_nonce + 3;
        payload.battle_count = 4;
        payload.zone_progress_delta = vec![ZoneProgressDeltaEntryFixture {
            zone_id: second_zone_id,
            new_state: ZONE_STATE_UNLOCKED,
        }];
        payload.encounter_histogram = vec![
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: fixtures.enemy.enemy_archetype_id,
                count: 1,
            },
            EncounterCountEntryFixture {
                zone_id: second_zone_id,
                enemy_archetype_id: second_enemy_id,
                count: 3,
            },
        ];
    });

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let mut instructions = harness
        .build_settlement_request_instructions(&fixtures, &pre_instructions)
        .expect("instructions should build");
    let settlement_ix = settlement_instruction_mut(&mut instructions);
    let index = find_account_index(settlement_ix, zone_enemy_set_pubkey(second_zone_id));
    settlement_ix.accounts.remove(index);

    let authority = canonical_authority_keypair();
    let err = harness
        .submit_versioned_transaction_with_signers(&instructions, &authority, &[&authority])
        .expect_err("missing zone enemy set should fail");

    assert_err_contains(
        err,
        "The settlement batch is missing a required zone enemy set account",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_missing_enemy_registry_account() {
    let mut fixtures = unique_integration_fixture_set();
    let second_enemy_id = fixtures.enemy.enemy_archetype_id + 501;
    fixtures.zone.allowed_enemy_archetype_ids =
        vec![fixtures.enemy.enemy_archetype_id, second_enemy_id];

    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");
    harness
        .ensure_enemy_archetype_registry_entry(
            fixtures.program.program_config_pubkey,
            second_enemy_id,
            24,
        )
        .expect("second enemy registry should initialize");

    let fixtures = with_payload(&fixtures, 85, |payload| {
        payload.encounter_histogram = vec![
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: fixtures.enemy.enemy_archetype_id,
                count: 1,
            },
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: second_enemy_id,
                count: 2,
            },
        ];
    });

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let mut instructions = harness
        .build_settlement_request_instructions(&fixtures, &pre_instructions)
        .expect("instructions should build");
    let settlement_ix = settlement_instruction_mut(&mut instructions);
    let index = find_account_index(settlement_ix, enemy_archetype_pubkey(second_enemy_id));
    settlement_ix.accounts.remove(index);

    let authority = canonical_authority_keypair();
    let err = harness
        .submit_versioned_transaction_with_signers(&instructions, &authority, &[&authority])
        .expect_err("missing enemy registry should fail");

    assert_err_contains(
        err,
        "The settlement batch is missing a required enemy archetype registry account",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_enemy_not_in_zone_membership_set() {
    let fixtures = unique_integration_fixture_set();
    let second_enemy_id = fixtures.enemy.enemy_archetype_id + 601;
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");
    harness
        .ensure_enemy_archetype_registry_entry(
            fixtures.program.program_config_pubkey,
            second_enemy_id,
            24,
        )
        .expect("second enemy registry should initialize");

    let fixtures = with_payload(&fixtures, 85, |payload| {
        payload.encounter_histogram = vec![
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: fixtures.enemy.enemy_archetype_id,
                count: 1,
            },
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: second_enemy_id,
                count: 2,
            },
        ];
    });

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("enemy outside zone membership should fail");

    assert_err_contains(
        err,
        "The settlement batch references an enemy that is not legal for the zone",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_duplicate_registry_account() {
    let fixtures = unique_integration_fixture_set();
    let second_zone_id = fixtures.zone.zone_id + 51;
    let second_enemy_id = fixtures.enemy.enemy_archetype_id + 702;
    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");
    ensure_zone_bundle(
        &harness,
        &fixtures,
        second_zone_id,
        100,
        100,
        vec![second_enemy_id],
        &[(second_enemy_id, 15)],
    );

    let fixtures = with_payload(&fixtures, 55, |payload| {
        payload.end_nonce = payload.start_nonce + 3;
        payload.battle_count = 4;
        payload.zone_progress_delta = vec![ZoneProgressDeltaEntryFixture {
            zone_id: second_zone_id,
            new_state: ZONE_STATE_UNLOCKED,
        }];
        payload.encounter_histogram = vec![
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: fixtures.enemy.enemy_archetype_id,
                count: 1,
            },
            EncounterCountEntryFixture {
                zone_id: second_zone_id,
                enemy_archetype_id: second_enemy_id,
                count: 3,
            },
        ];
    });

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let mut instructions = harness
        .build_settlement_request_instructions(&fixtures, &pre_instructions)
        .expect("instructions should build");
    let settlement_ix = settlement_instruction_mut(&mut instructions);
    let source_index =
        find_account_index(settlement_ix, zone_registry_pubkey(fixtures.zone.zone_id));
    let target_index = find_account_index(settlement_ix, zone_registry_pubkey(second_zone_id));
    let duplicate = settlement_ix.accounts[source_index].clone();
    settlement_ix.accounts.insert(target_index, duplicate);

    let authority = canonical_authority_keypair();
    let err = harness
        .submit_versioned_transaction_with_signers(&instructions, &authority, &[&authority])
        .expect_err("duplicate registry account should fail");

    assert_err_contains(
        err,
        "Settlement remaining accounts must be supplied in canonical grouped ascending order",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_out_of_order_registry_group() {
    let mut fixtures = unique_integration_fixture_set();
    let second_enemy_id = fixtures.enemy.enemy_archetype_id + 801;
    fixtures.zone.allowed_enemy_archetype_ids =
        vec![fixtures.enemy.enemy_archetype_id, second_enemy_id];

    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");
    harness
        .ensure_enemy_archetype_registry_entry(
            fixtures.program.program_config_pubkey,
            second_enemy_id,
            24,
        )
        .expect("second enemy registry should initialize");

    let fixtures = with_payload(&fixtures, 85, |payload| {
        payload.encounter_histogram = vec![
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: fixtures.enemy.enemy_archetype_id,
                count: 1,
            },
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: second_enemy_id,
                count: 2,
            },
        ];
    });

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let mut instructions = harness
        .build_settlement_request_instructions(&fixtures, &pre_instructions)
        .expect("instructions should build");
    let settlement_ix = settlement_instruction_mut(&mut instructions);
    let first_index = find_account_index(
        settlement_ix,
        enemy_archetype_pubkey(fixtures.enemy.enemy_archetype_id),
    );
    let second_index = find_account_index(settlement_ix, enemy_archetype_pubkey(second_enemy_id));
    settlement_ix.accounts.swap(first_index, second_index);

    let authority = canonical_authority_keypair();
    let err = harness
        .submit_versioned_transaction_with_signers(&instructions, &authority, &[&authority])
        .expect_err("out-of-order registry group should fail");

    assert_err_contains(
        err,
        "Settlement remaining accounts must be supplied in canonical grouped ascending order",
    );
}

#[test]
fn test_apply_battle_settlement_batch_v1_rejects_mixed_exp_overflow() {
    let mut fixtures = unique_integration_fixture_set();
    let second_enemy_id = fixtures.enemy.enemy_archetype_id + 901;
    fixtures.zone.allowed_enemy_archetype_ids =
        vec![fixtures.enemy.enemy_archetype_id, second_enemy_id];
    fixtures.zone.exp_multiplier_num = u16::MAX;
    fixtures.zone.exp_multiplier_den = 1;
    fixtures.enemy.exp_reward_base = u32::MAX;

    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&fixtures)
        .expect("fixture state should bootstrap");
    harness
        .ensure_enemy_archetype_registry_entry(
            fixtures.program.program_config_pubkey,
            second_enemy_id,
            u32::MAX,
        )
        .expect("second enemy registry should initialize");

    let fixtures = with_payload(&fixtures, u32::MAX, |payload| {
        payload.end_nonce = 32;
        payload.battle_count = 32;
        payload.last_battle_ts = payload.first_battle_ts + 93;
        payload.encounter_histogram = vec![
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: fixtures.enemy.enemy_archetype_id,
                count: 16,
            },
            EncounterCountEntryFixture {
                zone_id: fixtures.zone.zone_id,
                enemy_archetype_id: second_enemy_id,
                count: 16,
            },
        ];
    });

    let pre_instructions = build_dual_ed25519_verification_instructions(&fixtures);
    let err = harness
        .submit_settlement_with_pre_instructions(&fixtures, &pre_instructions)
        .expect_err("mixed exp overflow should fail");

    assert_err_contains(err, "Settlement math overflowed");
}

#[test]
fn test_apply_battle_settlement_batch_v1_supports_sequential_mixed_batches() {
    let mut base = unique_integration_fixture_set();
    let second_enemy_id = base.enemy.enemy_archetype_id + 1001;
    let second_zone_id = base.zone.zone_id + 61;
    let third_enemy_id = base.enemy.enemy_archetype_id + 1003;
    base.zone.allowed_enemy_archetype_ids = vec![base.enemy.enemy_archetype_id, second_enemy_id];

    let harness = LocalnetRelayerHarness::new().expect("localnet harness should initialize");
    harness
        .bootstrap_slice1_fixture_state(&base)
        .expect("fixture state should bootstrap");
    harness
        .ensure_enemy_archetype_registry_entry(
            base.program.program_config_pubkey,
            second_enemy_id,
            24,
        )
        .expect("second enemy registry should initialize");
    ensure_zone_bundle(
        &harness,
        &base,
        second_zone_id,
        200,
        100,
        vec![third_enemy_id],
        &[(third_enemy_id, 12)],
    );

    let batch_one_pre_instructions = build_dual_ed25519_verification_instructions(&base);
    let tx = harness
        .submit_settlement_with_pre_instructions(&base, &batch_one_pre_instructions)
        .expect("first batch should succeed");
    harness
        .assert_signature_confirmed(&tx)
        .expect("first batch should confirm");

    let batch_two = with_payload(&base, 78, |payload| {
        payload.batch_id = 2;
        payload.start_nonce = base.batch.payload.end_nonce + 1;
        payload.end_nonce = payload.start_nonce + 2;
        payload.battle_count = 3;
        payload.first_battle_ts = base.batch.payload.last_battle_ts + 60;
        payload.last_battle_ts = payload.first_battle_ts + 120;
        payload.start_state_hash = base.batch.payload.end_state_hash;
        payload.end_state_hash = fixture_end_state_hash(
            base.character.character_root_pubkey,
            base.character.character_id,
            payload.batch_id,
            payload.end_nonce,
            payload.last_battle_ts,
        );
        payload.zone_progress_delta = vec![ZoneProgressDeltaEntryFixture {
            zone_id: second_zone_id,
            new_state: ZONE_STATE_UNLOCKED,
        }];
        payload.encounter_histogram = vec![
            EncounterCountEntryFixture {
                zone_id: base.zone.zone_id,
                enemy_archetype_id: second_enemy_id,
                count: 1,
            },
            EncounterCountEntryFixture {
                zone_id: second_zone_id,
                enemy_archetype_id: third_enemy_id,
                count: 2,
            },
        ];
    });

    let batch_two_pre_instructions = build_dual_ed25519_verification_instructions(&batch_two);
    let tx = harness
        .submit_settlement_with_pre_instructions(&batch_two, &batch_two_pre_instructions)
        .expect("second mixed batch should succeed");
    harness
        .assert_signature_confirmed(&tx)
        .expect("second mixed batch should confirm");

    let character_stats = harness
        .fetch_anchor_account::<runana_program::CharacterStatsAccount>(
            batch_two.character.character_stats_pubkey,
        )
        .expect("character stats fetch should succeed")
        .expect("character stats should exist");
    let character_world_progress = harness
        .fetch_anchor_account::<runana_program::CharacterWorldProgressAccount>(
            batch_two.character.character_world_progress_pubkey,
        )
        .expect("world progress fetch should succeed")
        .expect("world progress should exist");
    let cursor = harness
        .fetch_anchor_account::<runana_program::CharacterSettlementBatchCursorAccount>(
            batch_two.character.character_settlement_batch_cursor_pubkey,
        )
        .expect("cursor fetch should succeed")
        .expect("cursor should exist");

    assert_eq!(character_stats.total_exp, 153);
    assert_eq!(
        character_world_progress.highest_unlocked_zone_id,
        second_zone_id
    );
    assert_eq!(cursor.last_committed_batch_id, 2);
    assert_eq!(
        cursor.last_committed_end_nonce,
        batch_two.batch.payload.end_nonce
    );
    assert_eq!(
        cursor.last_committed_state_hash,
        batch_two.batch.payload.end_state_hash
    );
}
