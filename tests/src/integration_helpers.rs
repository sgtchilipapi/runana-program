use std::{error::Error, path::PathBuf, rc::Rc, thread::sleep, time::Duration};

#[allow(deprecated)]
use anchor_client::{
    anchor_lang::AccountDeserialize,
    solana_sdk::{
        account::Account,
        address_lookup_table::{self, AddressLookupTableAccount},
        commitment_config::CommitmentConfig,
        instruction::Instruction,
        message::{v0, VersionedMessage},
        pubkey::Pubkey,
        signature::{read_keypair_file, Signature},
        signer::{keypair::Keypair, Signer},
        transaction::{Transaction, VersionedTransaction},
    },
    Client, ClientError, Cluster, Program,
};

use crate::fixtures::{
    apply_battle_settlement_batch_v1_args_for_fixture, canonical_admin_keypair,
    canonical_authority_keypair, canonical_server_signer_keypair,
    create_character_args_for_fixture, initialize_enemy_archetype_registry_args_for_fixture,
    initialize_program_config_args_for_fixture, initialize_zone_enemy_set_args_for_fixture,
    initialize_zone_registry_args_for_fixture, CanonicalFixtureSet,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedEd25519Message {
    pub signer_pubkey: Pubkey,
    pub message: Vec<u8>,
    pub signature: [u8; 64],
}

pub struct LocalnetRelayerHarness {
    payer: Rc<Keypair>,
    program: Program<Rc<Keypair>>,
}

impl LocalnetRelayerHarness {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let payer = Rc::new(read_keypair_file(anchor_wallet_path()?)?);
        let client = Client::new_with_options(
            Cluster::Localnet,
            payer.clone(),
            CommitmentConfig::confirmed(),
        );
        let program = client.program(runana_program::id())?;
        Ok(Self { payer, program })
    }

    pub fn bootstrap_slice1_fixture_state(
        &self,
        fixtures: &CanonicalFixtureSet,
    ) -> Result<(), Box<dyn Error>> {
        self.ensure_program_config(fixtures)?;
        self.ensure_zone_registry(fixtures)?;
        self.ensure_zone_enemy_set(fixtures)?;
        self.ensure_enemy_archetype_registry(fixtures)?;
        self.ensure_character(fixtures)?;
        Ok(())
    }

    pub fn build_settlement_request_instructions(
        &self,
        fixtures: &CanonicalFixtureSet,
        pre_instructions: &[Instruction],
    ) -> Result<Vec<Instruction>, ClientError> {
        let mut request = self.program.request();
        for ix in pre_instructions.iter().cloned() {
            request = request.instruction(ix);
        }

        request
            .accounts(runana_program::accounts::ApplyBattleSettlementBatchV1 {
                player_authority: fixtures.character.authority,
                instructions_sysvar: anchor_client::solana_sdk::sysvar::instructions::ID,
                program_config: fixtures.program.program_config_pubkey,
                character_root: fixtures.character.character_root_pubkey,
                character_stats: fixtures.character.character_stats_pubkey,
                character_world_progress: fixtures.character.character_world_progress_pubkey,
                character_zone_progress_page: fixtures
                    .character
                    .character_zone_progress_page_pubkey,
                zone_registry: fixtures.zone.zone_registry_pubkey,
                zone_enemy_set: fixtures.zone.zone_enemy_set_pubkey,
                enemy_archetype_registry: fixtures.enemy.enemy_archetype_pubkey,
                character_settlement_batch_cursor: fixtures
                    .character
                    .character_settlement_batch_cursor_pubkey,
            })
            .args(runana_program::instruction::ApplyBattleSettlementBatchV1 {
                args: apply_battle_settlement_batch_v1_args_for_fixture(fixtures),
            })
            .instructions()
    }

    pub fn submit_settlement_with_pre_instructions(
        &self,
        fixtures: &CanonicalFixtureSet,
        pre_instructions: &[Instruction],
    ) -> Result<Signature, Box<dyn Error>> {
        let instructions =
            self.build_settlement_request_instructions(fixtures, pre_instructions)?;
        let lookup_table = self.create_lookup_table_for_instructions(&instructions)?;
        let blockhash = self.program.rpc().get_latest_blockhash()?;
        let message = v0::Message::try_compile(
            &self.payer.pubkey(),
            &instructions,
            &[lookup_table],
            blockhash,
        )?;
        let transaction =
            VersionedTransaction::try_new(VersionedMessage::V0(message), &[self.payer.as_ref()])?;

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
                payer: self.payer.pubkey(),
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
        if self
            .fetch_anchor_account::<runana_program::ZoneRegistryAccount>(
                fixtures.zone.zone_registry_pubkey,
            )?
            .is_some()
        {
            return Ok(());
        }

        let admin = canonical_admin_keypair();
        self.program
            .request()
            .accounts(runana_program::accounts::InitializeZoneRegistry {
                payer: self.payer.pubkey(),
                admin_authority: admin.pubkey(),
                program_config: fixtures.program.program_config_pubkey,
                zone_registry: fixtures.zone.zone_registry_pubkey,
                system_program: anchor_client::solana_sdk::system_program::ID,
            })
            .args(runana_program::instruction::InitializeZoneRegistry {
                args: initialize_zone_registry_args_for_fixture(fixtures),
            })
            .signer(&admin)
            .send()?;

        Ok(())
    }

    fn ensure_zone_enemy_set(&self, fixtures: &CanonicalFixtureSet) -> Result<(), Box<dyn Error>> {
        if self
            .fetch_anchor_account::<runana_program::ZoneEnemySetAccount>(
                fixtures.zone.zone_enemy_set_pubkey,
            )?
            .is_some()
        {
            return Ok(());
        }

        let admin = canonical_admin_keypair();
        self.program
            .request()
            .accounts(runana_program::accounts::InitializeZoneEnemySet {
                payer: self.payer.pubkey(),
                admin_authority: admin.pubkey(),
                program_config: fixtures.program.program_config_pubkey,
                zone_enemy_set: fixtures.zone.zone_enemy_set_pubkey,
                system_program: anchor_client::solana_sdk::system_program::ID,
            })
            .args(runana_program::instruction::InitializeZoneEnemySet {
                args: initialize_zone_enemy_set_args_for_fixture(fixtures),
            })
            .signer(&admin)
            .send()?;

        Ok(())
    }

    fn ensure_enemy_archetype_registry(
        &self,
        fixtures: &CanonicalFixtureSet,
    ) -> Result<(), Box<dyn Error>> {
        if self
            .fetch_anchor_account::<runana_program::EnemyArchetypeRegistryAccount>(
                fixtures.enemy.enemy_archetype_pubkey,
            )?
            .is_some()
        {
            return Ok(());
        }

        let admin = canonical_admin_keypair();
        self.program
            .request()
            .accounts(runana_program::accounts::InitializeEnemyArchetypeRegistry {
                payer: self.payer.pubkey(),
                admin_authority: admin.pubkey(),
                program_config: fixtures.program.program_config_pubkey,
                enemy_archetype_registry: fixtures.enemy.enemy_archetype_pubkey,
                system_program: anchor_client::solana_sdk::system_program::ID,
            })
            .args(
                runana_program::instruction::InitializeEnemyArchetypeRegistry {
                    args: initialize_enemy_archetype_registry_args_for_fixture(fixtures),
                },
            )
            .signer(&admin)
            .send()?;

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
        self.program
            .request()
            .accounts(runana_program::accounts::CreateCharacter {
                payer: self.payer.pubkey(),
                authority: authority.pubkey(),
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
            .signer(&authority)
            .send()?;

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
            .get_slot_with_commitment(CommitmentConfig::processed())?
            .saturating_sub(1);
        let (create_ix, lookup_table_address) =
            address_lookup_table::instruction::create_lookup_table_signed(
                self.payer.pubkey(),
                self.payer.pubkey(),
                recent_slot,
            );
        self.send_legacy_transaction(&[create_ix])?;

        let extend_ix = address_lookup_table::instruction::extend_lookup_table(
            lookup_table_address,
            self.payer.pubkey(),
            Some(self.payer.pubkey()),
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
        let blockhash = self.program.rpc().get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            instructions,
            Some(&self.payer.pubkey()),
            &[self.payer.as_ref()],
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
