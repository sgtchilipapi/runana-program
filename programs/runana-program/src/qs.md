# `lib.rs` Table Of Contents

This is a beginner-friendly outline of [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs). It tells you what each major element is and what job it has in the program.

## 1. Imports And Program Identity

- [`use ...`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1)
  Brings Anchor and Solana helpers into scope.
- [`declare_id!(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L9)
  Declares the Solana program id. This is the on-chain address clients use to call the program.

## 2. PDA Seed Constants

- [`PROGRAM_CONFIG_SEED` through `ENEMY_ARCHETYPE_SEED`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L11)
  Byte-string constants used to derive PDA addresses for accounts.

## 3. General Constants

- [`ACCOUNT_VERSION_V1`, `CLUSTER_ID_LOCALNET`, `ZONE_STATE_*`, `ZONE_PAGE_WIDTH`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L21)
  Project-level constants used when initializing state and validating settlement data.
- [`ED25519_*` constants](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L27)
  Constants used to parse and validate the native ed25519 verification instructions.

## 4. Program Module And Instruction Handlers

- [`#[program] pub mod runana_program`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L34)
  The Anchor program module. Functions inside it are on-chain instructions clients can call.

### 4.1 `initialize_program_config`

- [`initialize_program_config(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L38)
  Creates and fills the global `ProgramConfigAccount` with admin and policy settings.

### 4.2 `initialize_zone_registry`

- [`initialize_zone_registry(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L54)
  Creates and fills the zone registry account for one zone, including EXP multiplier settings.

### 4.3 `initialize_zone_enemy_set`

- [`initialize_zone_enemy_set(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L72)
  Creates and fills the mapping that says which enemy archetype is allowed for a zone.

### 4.4 `initialize_enemy_archetype_registry`

- [`initialize_enemy_archetype_registry(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L84)
  Creates and fills the registry account for one enemy archetype, including base EXP reward.

### 4.5 `create_character`

- [`create_character(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L96)
  Creates and initializes the character’s on-chain state bundle:
  `CharacterRootAccount`, stats, world progress, zone progress page, and settlement cursor.

### 4.6 `apply_battle_settlement_batch_v1`

- [`apply_battle_settlement_batch_v1(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L153)
  The Slice 1 settlement instruction. It validates the settlement batch and then applies EXP and cursor updates.

## 5. Accounts Context Structs

These are Anchor `#[derive(Accounts)]` structs. They describe which accounts an instruction needs and how Anchor should validate or create them.

### 5.1 `InitializeProgramConfig`

- [`InitializeProgramConfig`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L206)
  Accounts needed to create the `ProgramConfigAccount`.

### 5.2 `InitializeZoneRegistry`

- [`InitializeZoneRegistry`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L223)
  Accounts needed to create a `ZoneRegistryAccount`.

### 5.3 `InitializeZoneEnemySet`

- [`InitializeZoneEnemySet`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L246)
  Accounts needed to create a `ZoneEnemySetAccount`.

### 5.4 `InitializeEnemyArchetypeRegistry`

- [`InitializeEnemyArchetypeRegistry`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L269)
  Accounts needed to create an `EnemyArchetypeRegistryAccount`.

### 5.5 `CreateCharacter`

- [`CreateCharacter`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L292)
  Accounts needed to create the initial character state bundle.

### 5.6 `ApplyBattleSettlementBatchV1`

- [`ApplyBattleSettlementBatchV1`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L344)
  Accounts needed to validate and apply a settlement batch.

## 6. On-Chain State Account Structs

These structs define the actual data stored on-chain in program-owned accounts.

### 6.1 Global Config

- [`ProgramConfigAccount`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L366)
  Global program settings like admin authority, trusted server signer, and settlement policy limits.

### 6.2 Character Accounts

- [`CharacterRootAccount`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L382)
  The main identity/ownership record for a character.
- [`CharacterStatsAccount`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L395)
  Stores the character’s progression stats, currently level and total EXP.
- [`CharacterWorldProgressAccount`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L408)
  Stores summarized world progression like highest unlocked and cleared zone.
- [`CharacterZoneProgressPageAccount`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L421)
  Stores per-zone progress states for one page of zones.
- [`CharacterSettlementBatchCursorAccount`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L471)
  Stores the last committed settlement checkpoint so future batches can chain from it.

### 6.3 Registry Accounts

- [`ZoneRegistryAccount`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L434)
  Stores zone metadata relevant to settlement, currently EXP multiplier settings.
- [`ZoneEnemySetAccount`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L447)
  Stores which enemy archetype is allowed for a zone in this MVP path.
- [`EnemyArchetypeRegistryAccount`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L459)
  Stores metadata for one enemy archetype, currently base EXP reward.

## 7. Instruction Argument Structs

These are the serialized instruction arguments that clients send into the program.

- [`InitializeProgramConfigArgs`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L488)
  Inputs for config initialization.
- [`InitializeZoneRegistryArgs`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L496)
  Inputs for zone registry initialization.
- [`InitializeZoneEnemySetArgs`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L503)
  Inputs for zone-enemy mapping initialization.
- [`InitializeEnemyArchetypeRegistryArgs`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L509)
  Inputs for enemy archetype initialization.
- [`CreateCharacterArgs`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L515)
  Inputs for character creation.
- [`ApplyBattleSettlementBatchV1Args`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L523)
  Wrapper around the settlement payload.

## 8. Settlement Payload Types

These structs define the data inside a settlement batch.

- [`SettlementBatchPayloadV1`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L528)
  The main settlement payload sent into `ApplyBattleSettlementBatchV1`.
- [`EncounterCountEntry`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L548)
  One histogram entry: which zone, which enemy archetype, how many battles.
- [`ZoneProgressDeltaEntry`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L555)
  One requested zone progression update.
- `SettlementBatchPayloadPreimageV1` at [lib.rs](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L560)
  Internal helper struct used to recompute the canonical `batch_hash`.

## 9. Helper Functions

These are private helper functions used by the main instruction handlers.

### 9.1 State Hash Helper

- [`compute_genesis_state_hash(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L601)
  Computes the initial settlement state hash for a newly created character.

### 9.2 Account Validation

- [`verify_canonical_account_addresses(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L611)
  Re-derives PDAs and checks that the passed accounts are the expected canonical addresses.
- [`verify_character_binding(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L729)
  Checks that the settlement payload, authority, and character-linked accounts all belong together.

### 9.3 Signature Validation

- [`verify_ed25519_preinstructions(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L770)
  Reads `SysvarInstructions`, checks the two preceding ed25519 verification instructions, and matches them against expected server/player messages.
- [`verify_ed25519_instruction_payload(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L832)
  Low-level parser/checker for one ed25519 verification instruction.
- [`read_u16_le(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L910)
  Tiny helper for reading little-endian integers from raw instruction bytes.

### 9.4 Settlement Payload Validation

- [`verify_nonce_range(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L915)
  Checks that `battle_count` matches the nonce range.
- [`verify_histogram_count(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L930)
  Checks that the encounter histogram totals add up to `battle_count`.
- [`verify_batch_hash(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L947)
  Recomputes the canonical `batch_hash` and compares it to the payload.
- [`verify_batch_continuity(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L962)
  Checks that this batch starts where the settlement cursor says it must start.

### 9.5 Settlement Application Helpers

- [`derive_exp_delta(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L991)
  Computes EXP from the encounter histogram and on-chain registry data.
- [`apply_zone_progress_delta(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1035)
  Applies zone progression changes to the page account and summary world progress account.

### 9.6 Canonical Message Builders

- [`canonical_server_attestation_message(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1068)
  Reconstructs the exact message the trusted server was expected to sign.
- [`canonical_player_authorization_message(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1097)
  Reconstructs the exact message the player authority was expected to sign.

### 9.7 Serialization Helpers

- [`put_zone_progress_delta_vec(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1117)
  Serializes zone progress entries into canonical bytes.
- [`put_encounter_histogram_vec(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1125)
  Serializes encounter histogram entries into canonical bytes.
- [`put_option_u32(...)`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1134)
  Serializes an optional `u32` into canonical bytes.

## 10. Error Enum

- [`SettlementError`](/home/paps/projects/runana-program/programs/runana-program/src/lib.rs#L1145)
  The program’s custom error codes. These are returned when validation or state application fails.
