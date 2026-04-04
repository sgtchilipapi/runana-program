use std::{error::Error, path::PathBuf, rc::Rc, thread::sleep, time::Duration};

#[allow(deprecated)]
use anchor_client::{
    anchor_lang::AccountDeserialize,
    solana_sdk::{
        account::Account,
        address_lookup_table::{self, AddressLookupTableAccount},
        commitment_config::CommitmentConfig,
        instruction::{AccountMeta, Instruction},
        message::{v0, VersionedMessage},
        pubkey::Pubkey,
        signature::{read_keypair_file, Signature},
        signer::{keypair::Keypair, Signer},
        system_instruction,
        transaction::{Transaction, VersionedTransaction},
    },
    Client, ClientError, Cluster, Program,
};

use crate::fixtures::{
    apply_battle_settlement_batch_v1_args_for_fixture, canonical_admin_keypair,
    canonical_authority_keypair, canonical_server_signer_keypair,
    create_character_args_for_fixture, initialize_character_zone_progress_page_args,
    initialize_enemy_archetype_registry_args_for_fixture,
    initialize_program_config_args_for_fixture, initialize_season_policy_args_for_fixture,
    initialize_zone_registry_args_for_fixture, CanonicalFixtureSet, CHARACTER_ZONE_PROGRESS_SEED,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedEd25519Message {
    pub signer_pubkey: Pubkey,
    pub message: Vec<u8>,
    pub signature: [u8; 64],
}

pub struct LocalnetRelayerHarness {
    relayer: Rc<Keypair>,
    program: Program<Rc<Keypair>>,
}

const MIN_PLAYER_LAMPORTS: u64 = 50_000_000;

impl LocalnetRelayerHarness {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let relayer = Rc::new(read_keypair_file(anchor_wallet_path()?)?);
        let client = Client::new_with_options(
            Cluster::Localnet,
            relayer.clone(),
            CommitmentConfig::confirmed(),
        );
        let program = client.program(runana_program::id())?;
        Ok(Self { relayer, program })
    }

    pub fn bootstrap_slice1_fixture_state(
        &self,
        fixtures: &CanonicalFixtureSet,
    ) -> Result<(), Box<dyn Error>> {
        self.bootstrap_slice1_static_fixture_state(fixtures)?;
        self.ensure_character(fixtures)?;
        Ok(())
    }

    pub fn bootstrap_slice1_static_fixture_state(
        &self,
        fixtures: &CanonicalFixtureSet,
    ) -> Result<(), Box<dyn Error>> {
        self.ensure_program_config(fixtures)?;
        self.ensure_season_policy(fixtures)?;
        self.ensure_zone_registry(fixtures)?;
        self.ensure_zone_enemy_set(fixtures)?;
        self.ensure_enemy_archetype_registry(fixtures)?;
        Ok(())
    }

    pub fn build_settlement_request_instructions(
        &self,
        fixtures: &CanonicalFixtureSet,
        pre_instructions: &[Instruction],
    ) -> Result<Vec<Instruction>, ClientError> {
        self.build_settlement_request_instructions_with_accounts_args_and_extra_pages(
            fixtures,
            fixtures.character.authority,
            apply_battle_settlement_batch_v1_args_for_fixture(fixtures),
            pre_instructions,
            &[],
        )
    }

    pub fn build_settlement_request_instructions_with_accounts_and_args(
        &self,
        fixtures: &CanonicalFixtureSet,
        player_authority: Pubkey,
        args: runana_program::ApplyBattleSettlementBatchV1Args,
        pre_instructions: &[Instruction],
    ) -> Result<Vec<Instruction>, ClientError> {
        self.build_settlement_request_instructions_with_accounts_args_and_extra_pages(
            fixtures,
            player_authority,
            args,
            pre_instructions,
            &[],
        )
    }

    pub fn build_settlement_request_instructions_with_accounts_args_and_extra_pages(
        &self,
        fixtures: &CanonicalFixtureSet,
        player_authority: Pubkey,
        args: runana_program::ApplyBattleSettlementBatchV1Args,
        pre_instructions: &[Instruction],
        extra_zone_progress_pages: &[Pubkey],
    ) -> Result<Vec<Instruction>, ClientError> {
        let mut request = self.program.request();
        for ix in pre_instructions.iter().cloned() {
            request = request.instruction(ix);
        }
        let payload = args.payload.clone();

        let mut instructions = request
            .accounts(runana_program::accounts::ApplyBattleSettlementBatchV1 {
                player_authority,
                instructions_sysvar: anchor_client::solana_sdk::sysvar::instructions::ID,
                program_config: fixtures.program.program_config_pubkey,
                character_root: fixtures.character.character_root_pubkey,
                character_stats: fixtures.character.character_stats_pubkey,
                character_world_progress: fixtures.character.character_world_progress_pubkey,
                character_zone_progress_page: fixtures
                    .character
                    .character_zone_progress_page_pubkey,
                season_policy: fixtures.season.season_policy_pubkey,
                character_settlement_batch_cursor: fixtures
                    .character
                    .character_settlement_batch_cursor_pubkey,
            })
            .args(runana_program::instruction::ApplyBattleSettlementBatchV1 { args })
            .instructions()?;

        if let Some(settlement_ix) = instructions.last_mut() {
            let zone_progress_pages = if extra_zone_progress_pages.is_empty() {
                canonical_extra_zone_progress_page_pubkeys(
                    fixtures.character.character_root_pubkey,
                    &payload,
                )
            } else {
                extra_zone_progress_pages.to_vec()
            };

            for page in zone_progress_pages {
                settlement_ix.accounts.push(AccountMeta::new(page, false));
            }
            for zone_registry in referenced_zone_registry_pubkeys(&payload) {
                settlement_ix
                    .accounts
                    .push(AccountMeta::new_readonly(zone_registry, false));
            }
            for zone_enemy_set in referenced_zone_enemy_set_pubkeys(&payload) {
                settlement_ix
                    .accounts
                    .push(AccountMeta::new_readonly(zone_enemy_set, false));
            }
            for enemy_archetype in referenced_enemy_archetype_pubkeys(&payload) {
                settlement_ix
                    .accounts
                    .push(AccountMeta::new_readonly(enemy_archetype, false));
            }
        }

        Ok(instructions)
    }

    pub fn submit_settlement_with_pre_instructions(
        &self,
        fixtures: &CanonicalFixtureSet,
        pre_instructions: &[Instruction],
    ) -> Result<Signature, Box<dyn Error>> {
        let instructions =
            self.build_settlement_request_instructions(fixtures, pre_instructions)?;
        let authority = canonical_authority_keypair();
        self.submit_versioned_transaction_with_signers(&instructions, &authority, &[&authority])
    }

    pub fn submit_settlement_with_pre_instructions_and_extra_pages(
        &self,
        fixtures: &CanonicalFixtureSet,
        pre_instructions: &[Instruction],
        extra_zone_progress_pages: &[Pubkey],
    ) -> Result<Signature, Box<dyn Error>> {
        let instructions = self
            .build_settlement_request_instructions_with_accounts_args_and_extra_pages(
                fixtures,
                fixtures.character.authority,
                apply_battle_settlement_batch_v1_args_for_fixture(fixtures),
                pre_instructions,
                extra_zone_progress_pages,
            )?;
        let authority = canonical_authority_keypair();
        self.submit_versioned_transaction_with_signers(&instructions, &authority, &[&authority])
    }

    pub fn submit_versioned_transaction_with_signers(
        &self,
        instructions: &[Instruction],
        fee_payer: &Keypair,
        signers: &[&Keypair],
    ) -> Result<Signature, Box<dyn Error>> {
        let lookup_table = self.create_lookup_table_for_instructions(instructions)?;
        self.ensure_wallet_funded(fee_payer)?;
        let blockhash = self.program.rpc().get_latest_blockhash()?;
        let message = v0::Message::try_compile(
            &fee_payer.pubkey(),
            instructions,
            &[lookup_table],
            blockhash,
        )?;
        let transaction = VersionedTransaction::try_new(VersionedMessage::V0(message), signers)?;

        Ok(self
            .program
            .rpc()
            .send_and_confirm_transaction(&transaction)?)
    }

    pub fn assert_signature_confirmed(&self, signature: &Signature) -> Result<(), Box<dyn Error>> {
        let statuses = self.program.rpc().get_signature_statuses(&[*signature])?;
        let status = statuses
            .value
            .into_iter()
            .next()
            .flatten()
            .ok_or("transaction signature not found on localnet")?;

        if let Some(err) = status.err {
            return Err(format!("transaction failed: {err:?}").into());
        }

        Ok(())
    }

    pub fn fetch_account(&self, address: Pubkey) -> Result<Option<Account>, Box<dyn Error>> {
        match self.program.rpc().get_account(&address) {
            Ok(account) => Ok(Some(account)),
            Err(err) if err.to_string().contains("AccountNotFound") => Ok(None),
            Err(err) => Err(Box::new(err)),
        }
    }

    pub fn fetch_anchor_account<T>(&self, address: Pubkey) -> Result<Option<T>, Box<dyn Error>>
    where
        T: AccountDeserialize,
    {
        let Some(account) = self.fetch_account(address)? else {
            return Ok(None);
        };
        let mut data = account.data.as_slice();
        Ok(Some(T::try_deserialize(&mut data)?))
    }

    pub fn ensure_zone_registry_entry(
        &self,
        program_config_pubkey: Pubkey,
        zone_id: u16,
        exp_multiplier_num: u16,
        exp_multiplier_den: u16,
    ) -> Result<Pubkey, Box<dyn Error>> {
        let zone_registry_pubkey = Pubkey::find_program_address(
            &[b"zone_registry", &zone_id.to_le_bytes()],
            &runana_program::id(),
        )
        .0;
        if self
            .fetch_anchor_account::<runana_program::ZoneRegistryAccount>(zone_registry_pubkey)?
            .is_some()
        {
            return Ok(zone_registry_pubkey);
        }

        let admin = canonical_admin_keypair();
        self.program
            .request()
            .accounts(runana_program::accounts::InitializeZoneRegistry {
                payer: self.relayer.pubkey(),
                admin_authority: admin.pubkey(),
                program_config: program_config_pubkey,
                zone_registry: zone_registry_pubkey,
                system_program: anchor_client::solana_sdk::system_program::ID,
            })
            .args(runana_program::instruction::InitializeZoneRegistry {
                args: runana_program::InitializeZoneRegistryArgs {
                    zone_id,
                    exp_multiplier_num,
                    exp_multiplier_den,
                },
            })
            .signer(&admin)
            .send()?;

        Ok(zone_registry_pubkey)
    }

    pub fn upsert_zone_enemy_set_entry(
        &self,
        program_config_pubkey: Pubkey,
        zone_id: u16,
        allowed_enemy_archetype_ids: Vec<u16>,
    ) -> Result<Pubkey, Box<dyn Error>> {
        let zone_enemy_set_pubkey = Pubkey::find_program_address(
            &[b"zone_enemy_set", &zone_id.to_le_bytes()],
            &runana_program::id(),
        )
        .0;
        let admin = canonical_admin_keypair();

        if self
            .fetch_anchor_account::<runana_program::ZoneEnemySetAccount>(zone_enemy_set_pubkey)?
            .is_some()
        {
            self.program
                .request()
                .accounts(runana_program::accounts::UpdateZoneEnemySet {
                    admin_authority: admin.pubkey(),
                    program_config: program_config_pubkey,
                    zone_enemy_set: zone_enemy_set_pubkey,
                })
                .args(runana_program::instruction::UpdateZoneEnemySet {
                    args: runana_program::UpdateZoneEnemySetArgs {
                        zone_id,
                        allowed_enemy_archetype_ids,
                    },
                })
                .signer(&admin)
                .send()?;
            return Ok(zone_enemy_set_pubkey);
        }

        self.program
            .request()
            .accounts(runana_program::accounts::InitializeZoneEnemySet {
                payer: self.relayer.pubkey(),
                admin_authority: admin.pubkey(),
                program_config: program_config_pubkey,
                zone_enemy_set: zone_enemy_set_pubkey,
                system_program: anchor_client::solana_sdk::system_program::ID,
            })
            .args(runana_program::instruction::InitializeZoneEnemySet {
                args: runana_program::InitializeZoneEnemySetArgs {
                    zone_id,
                    allowed_enemy_archetype_ids,
                },
            })
            .signer(&admin)
            .send()?;

        Ok(zone_enemy_set_pubkey)
    }

    pub fn ensure_enemy_archetype_registry_entry(
        &self,
        program_config_pubkey: Pubkey,
        enemy_archetype_id: u16,
        exp_reward_base: u32,
    ) -> Result<Pubkey, Box<dyn Error>> {
        let enemy_archetype_pubkey = Pubkey::find_program_address(
            &[b"enemy_archetype", &enemy_archetype_id.to_le_bytes()],
            &runana_program::id(),
        )
        .0;
        if self
            .fetch_anchor_account::<runana_program::EnemyArchetypeRegistryAccount>(
                enemy_archetype_pubkey,
            )?
            .is_some()
        {
            return Ok(enemy_archetype_pubkey);
        }

        let admin = canonical_admin_keypair();
        self.program
            .request()
            .accounts(runana_program::accounts::InitializeEnemyArchetypeRegistry {
                payer: self.relayer.pubkey(),
                admin_authority: admin.pubkey(),
                program_config: program_config_pubkey,
                enemy_archetype_registry: enemy_archetype_pubkey,
                system_program: anchor_client::solana_sdk::system_program::ID,
            })
            .args(
                runana_program::instruction::InitializeEnemyArchetypeRegistry {
                    args: runana_program::InitializeEnemyArchetypeRegistryArgs {
                        enemy_archetype_id,
                        exp_reward_base,
                    },
                },
            )
            .signer(&admin)
            .send()?;

        Ok(enemy_archetype_pubkey)
    }

    fn ensure_program_config(&self, fixtures: &CanonicalFixtureSet) -> Result<(), Box<dyn Error>> {
        if self
            .fetch_anchor_account::<runana_program::ProgramConfigAccount>(
                fixtures.program.program_config_pubkey,
            )?
            .is_some()
        {
            return Ok(());
        }

        let admin = canonical_admin_keypair();
        self.program
            .request()
            .accounts(runana_program::accounts::InitializeProgramConfig {
                payer: self.relayer.pubkey(),
                admin_authority: admin.pubkey(),
                program_config: fixtures.program.program_config_pubkey,
                system_program: anchor_client::solana_sdk::system_program::ID,
            })
            .args(runana_program::instruction::InitializeProgramConfig {
                args: initialize_program_config_args_for_fixture(fixtures),
            })
            .signer(&admin)
            .send()?;

        Ok(())
    }

    fn ensure_zone_registry(&self, fixtures: &CanonicalFixtureSet) -> Result<(), Box<dyn Error>> {
        let args = initialize_zone_registry_args_for_fixture(fixtures);
        self.ensure_zone_registry_entry(
            fixtures.program.program_config_pubkey,
            args.zone_id,
            args.exp_multiplier_num,
            args.exp_multiplier_den,
        )?;
        Ok(())
    }

    fn ensure_season_policy(&self, fixtures: &CanonicalFixtureSet) -> Result<(), Box<dyn Error>> {
        if self
            .fetch_anchor_account::<runana_program::SeasonPolicyAccount>(
                fixtures.season.season_policy_pubkey,
            )?
            .is_some()
        {
            return Ok(());
        }

        let admin = canonical_admin_keypair();
        self.program
            .request()
            .accounts(runana_program::accounts::InitializeSeasonPolicy {
                payer: self.relayer.pubkey(),
                admin_authority: admin.pubkey(),
                program_config: fixtures.program.program_config_pubkey,
                season_policy: fixtures.season.season_policy_pubkey,
                system_program: anchor_client::solana_sdk::system_program::ID,
            })
            .args(runana_program::instruction::InitializeSeasonPolicy {
                args: initialize_season_policy_args_for_fixture(fixtures),
            })
            .signer(&admin)
            .send()?;

        Ok(())
    }

    fn ensure_zone_enemy_set(&self, fixtures: &CanonicalFixtureSet) -> Result<(), Box<dyn Error>> {
        self.upsert_zone_enemy_set_entry(
            fixtures.program.program_config_pubkey,
            fixtures.zone.zone_id,
            fixtures.zone.allowed_enemy_archetype_ids.clone(),
        )?;
        Ok(())
    }

    fn ensure_enemy_archetype_registry(
        &self,
        fixtures: &CanonicalFixtureSet,
    ) -> Result<(), Box<dyn Error>> {
        let args = initialize_enemy_archetype_registry_args_for_fixture(fixtures);
        self.ensure_enemy_archetype_registry_entry(
            fixtures.program.program_config_pubkey,
            args.enemy_archetype_id,
            args.exp_reward_base,
        )?;
        Ok(())
    }

    fn ensure_character(&self, fixtures: &CanonicalFixtureSet) -> Result<(), Box<dyn Error>> {
        if self
            .fetch_anchor_account::<runana_program::CharacterRootAccount>(
                fixtures.character.character_root_pubkey,
            )?
            .is_some()
        {
            return Ok(());
        }

        let authority = canonical_authority_keypair();
        self.submit_create_character_with_signers(fixtures, &authority, &[&authority])?;
        Ok(())
    }

    pub fn ensure_character_zone_progress_page(
        &self,
        character_root_pubkey: Pubkey,
        authority: &Keypair,
        page_index: u16,
    ) -> Result<Pubkey, Box<dyn Error>> {
        let (character_zone_progress_page_pubkey, _) = Pubkey::find_program_address(
            &[
                CHARACTER_ZONE_PROGRESS_SEED,
                character_root_pubkey.as_ref(),
                &page_index.to_le_bytes(),
            ],
            &runana_program::id(),
        );

        if self
            .fetch_anchor_account::<runana_program::CharacterZoneProgressPageAccount>(
                character_zone_progress_page_pubkey,
            )?
            .is_some()
        {
            return Ok(character_zone_progress_page_pubkey);
        }

        self.ensure_wallet_funded(authority)?;
        let instructions = self
            .program
            .request()
            .accounts(
                runana_program::accounts::InitializeCharacterZoneProgressPage {
                    payer: authority.pubkey(),
                    authority: authority.pubkey(),
                    character_root: character_root_pubkey,
                    character_zone_progress_page: character_zone_progress_page_pubkey,
                    system_program: anchor_client::solana_sdk::system_program::ID,
                },
            )
            .args(
                runana_program::instruction::InitializeCharacterZoneProgressPage {
                    args: initialize_character_zone_progress_page_args(page_index),
                },
            )
            .instructions()?;

        self.send_legacy_transaction_with_signers(&instructions, authority, &[authority])?;
        Ok(character_zone_progress_page_pubkey)
    }

    pub fn submit_create_character_with_mismatched_payer(
        &self,
        fixtures: &CanonicalFixtureSet,
    ) -> Result<Signature, Box<dyn Error>> {
        let authority = canonical_authority_keypair();
        self.submit_create_character_with_signers(
            fixtures,
            self.relayer.as_ref(),
            &[self.relayer.as_ref(), &authority],
        )
    }

    pub fn submit_create_character_with_player_payer(
        &self,
        fixtures: &CanonicalFixtureSet,
    ) -> Result<Signature, Box<dyn Error>> {
        let authority = canonical_authority_keypair();
        self.submit_create_character_with_signers(fixtures, &authority, &[&authority])
    }

    pub fn build_create_character_instructions(
        &self,
        fixtures: &CanonicalFixtureSet,
        payer: Pubkey,
        authority: Pubkey,
    ) -> Result<Vec<Instruction>, ClientError> {
        self.program
            .request()
            .accounts(runana_program::accounts::CreateCharacter {
                payer,
                authority,
                character_root: fixtures.character.character_root_pubkey,
                character_stats: fixtures.character.character_stats_pubkey,
                character_world_progress: fixtures.character.character_world_progress_pubkey,
                character_zone_progress_page: fixtures
                    .character
                    .character_zone_progress_page_pubkey,
                character_settlement_batch_cursor: fixtures
                    .character
                    .character_settlement_batch_cursor_pubkey,
                system_program: anchor_client::solana_sdk::system_program::ID,
            })
            .args(runana_program::instruction::CreateCharacter {
                args: create_character_args_for_fixture(fixtures),
            })
            .instructions()
    }

    fn submit_create_character_with_signers(
        &self,
        fixtures: &CanonicalFixtureSet,
        fee_payer: &Keypair,
        signers: &[&Keypair],
    ) -> Result<Signature, Box<dyn Error>> {
        let authority = canonical_authority_keypair();
        self.ensure_wallet_funded(fee_payer)?;
        let instructions = self.build_create_character_instructions(
            fixtures,
            fee_payer.pubkey(),
            authority.pubkey(),
        )?;
        self.send_legacy_transaction_with_signers(&instructions, fee_payer, signers)
    }

    fn ensure_wallet_funded(&self, wallet: &Keypair) -> Result<(), Box<dyn Error>> {
        if wallet.pubkey() == self.relayer.pubkey() {
            return Ok(());
        }

        let current_balance = self.program.rpc().get_balance(&wallet.pubkey())?;
        if current_balance >= MIN_PLAYER_LAMPORTS {
            return Ok(());
        }

        let transfer_lamports = MIN_PLAYER_LAMPORTS - current_balance;
        let transfer_ix = system_instruction::transfer(
            &self.relayer.pubkey(),
            &wallet.pubkey(),
            transfer_lamports,
        );
        self.send_legacy_transaction(&[transfer_ix])?;
        Ok(())
    }

    #[allow(deprecated)]
    fn create_lookup_table_for_instructions(
        &self,
        instructions: &[Instruction],
    ) -> Result<AddressLookupTableAccount, Box<dyn Error>> {
        let lookup_addresses = collect_lookup_addresses(instructions);
        let recent_slot = self
            .program
            .rpc()
            .get_slot_with_commitment(CommitmentConfig::processed())?;
        self.wait_for_slot_advance(recent_slot)?;
        let (create_ix, lookup_table_address) =
            address_lookup_table::instruction::create_lookup_table_signed(
                self.relayer.pubkey(),
                self.relayer.pubkey(),
                recent_slot,
            );
        self.send_legacy_transaction(&[create_ix])?;

        let extend_ix = address_lookup_table::instruction::extend_lookup_table(
            lookup_table_address,
            self.relayer.pubkey(),
            Some(self.relayer.pubkey()),
            lookup_addresses,
        );
        self.send_legacy_transaction(&[extend_ix])?;
        let extend_slot = self
            .program
            .rpc()
            .get_slot_with_commitment(CommitmentConfig::processed())?;
        self.wait_for_slot_advance(extend_slot)?;

        let raw_account = self.program.rpc().get_account(&lookup_table_address)?;
        let lookup_table =
            address_lookup_table::state::AddressLookupTable::deserialize(&raw_account.data)?;

        Ok(AddressLookupTableAccount {
            key: lookup_table_address,
            addresses: lookup_table.addresses.to_vec(),
        })
    }

    fn send_legacy_transaction(
        &self,
        instructions: &[Instruction],
    ) -> Result<Signature, Box<dyn Error>> {
        self.send_legacy_transaction_with_signers(
            instructions,
            self.relayer.as_ref(),
            &[self.relayer.as_ref()],
        )
    }

    fn send_legacy_transaction_with_signers(
        &self,
        instructions: &[Instruction],
        fee_payer: &Keypair,
        signers: &[&Keypair],
    ) -> Result<Signature, Box<dyn Error>> {
        let blockhash = self.program.rpc().get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            instructions,
            Some(&fee_payer.pubkey()),
            signers,
            blockhash,
        );

        Ok(self
            .program
            .rpc()
            .send_and_confirm_transaction(&transaction)?)
    }

    fn wait_for_slot_advance(&self, prior_slot: u64) -> Result<(), Box<dyn Error>> {
        for _ in 0..20 {
            let current_slot = self.program.rpc().get_slot()?;
            if current_slot > prior_slot {
                return Ok(());
            }
            sleep(Duration::from_millis(100));
        }

        Err("address lookup table did not warm up on localnet".into())
    }
}

fn canonical_extra_zone_progress_page_pubkeys(
    character_root_pubkey: Pubkey,
    payload: &runana_program::SettlementBatchPayloadV1,
) -> Vec<Pubkey> {
    let mut page_indices = Vec::new();

    for entry in &payload.encounter_histogram {
        push_unique_sorted_u16(&mut page_indices, entry.zone_id / 256);
    }
    for entry in &payload.zone_progress_delta {
        push_unique_sorted_u16(&mut page_indices, entry.zone_id / 256);
    }

    page_indices
        .into_iter()
        .skip(1)
        .map(|page_index| {
            Pubkey::find_program_address(
                &[
                    CHARACTER_ZONE_PROGRESS_SEED,
                    character_root_pubkey.as_ref(),
                    &page_index.to_le_bytes(),
                ],
                &runana_program::id(),
            )
            .0
        })
        .collect()
}

fn referenced_zone_registry_pubkeys(
    payload: &runana_program::SettlementBatchPayloadV1,
) -> Vec<Pubkey> {
    let mut zone_ids = Vec::new();
    for entry in &payload.encounter_histogram {
        push_unique_sorted_u16(&mut zone_ids, entry.zone_id);
    }

    zone_ids
        .into_iter()
        .map(|zone_id| {
            Pubkey::find_program_address(
                &[b"zone_registry", &zone_id.to_le_bytes()],
                &runana_program::id(),
            )
            .0
        })
        .collect()
}

fn referenced_zone_enemy_set_pubkeys(
    payload: &runana_program::SettlementBatchPayloadV1,
) -> Vec<Pubkey> {
    let mut zone_ids = Vec::new();
    for entry in &payload.encounter_histogram {
        push_unique_sorted_u16(&mut zone_ids, entry.zone_id);
    }

    zone_ids
        .into_iter()
        .map(|zone_id| {
            Pubkey::find_program_address(
                &[b"zone_enemy_set", &zone_id.to_le_bytes()],
                &runana_program::id(),
            )
            .0
        })
        .collect()
}

fn referenced_enemy_archetype_pubkeys(
    payload: &runana_program::SettlementBatchPayloadV1,
) -> Vec<Pubkey> {
    let mut enemy_ids = Vec::new();
    for entry in &payload.encounter_histogram {
        push_unique_sorted_u16(&mut enemy_ids, entry.enemy_archetype_id);
    }

    enemy_ids
        .into_iter()
        .map(|enemy_archetype_id| {
            Pubkey::find_program_address(
                &[b"enemy_archetype", &enemy_archetype_id.to_le_bytes()],
                &runana_program::id(),
            )
            .0
        })
        .collect()
}

fn push_unique_sorted_u16(values: &mut Vec<u16>, next: u16) {
    if !values.contains(&next) {
        values.push(next);
        values.sort_unstable();
    }
}

pub fn sign_server_attestation(fixtures: &CanonicalFixtureSet) -> SignedEd25519Message {
    sign_message(
        &canonical_server_signer_keypair(),
        &fixtures.batch.server_attestation_message,
    )
}

pub fn sign_player_authorization(fixtures: &CanonicalFixtureSet) -> SignedEd25519Message {
    sign_message(
        &canonical_authority_keypair(),
        &fixtures.batch.player_authorization_message,
    )
}

pub fn sign_arbitrary_message(signer: &Keypair, message: &[u8]) -> SignedEd25519Message {
    sign_message(signer, message)
}

pub fn build_ed25519_verification_instruction(signed: &SignedEd25519Message) -> Instruction {
    solana_ed25519_program::new_ed25519_instruction_with_signature(
        &signed.message,
        &signed.signature,
        &signed.signer_pubkey.to_bytes(),
    )
}

pub fn build_dual_ed25519_verification_instructions(
    fixtures: &CanonicalFixtureSet,
) -> Vec<Instruction> {
    let server = sign_server_attestation(fixtures);
    let player = sign_player_authorization(fixtures);

    vec![
        build_ed25519_verification_instruction(&server),
        build_ed25519_verification_instruction(&player),
    ]
}

fn sign_message(signer: &Keypair, message: &[u8]) -> SignedEd25519Message {
    SignedEd25519Message {
        signer_pubkey: signer.pubkey(),
        message: message.to_vec(),
        signature: *signer.sign_message(message).as_array(),
    }
}

fn anchor_wallet_path() -> Result<PathBuf, Box<dyn Error>> {
    if let Ok(path) = std::env::var("ANCHOR_WALLET") {
        return Ok(PathBuf::from(path));
    }

    let home = std::env::var("HOME")?;
    Ok(PathBuf::from(home).join(".config/solana/id.json"))
}

fn collect_lookup_addresses(instructions: &[Instruction]) -> Vec<Pubkey> {
    let mut addresses = Vec::new();
    for instruction in instructions {
        for meta in &instruction.accounts {
            if !addresses.contains(&meta.pubkey) {
                addresses.push(meta.pubkey);
            }
        }
    }
    addresses
}
