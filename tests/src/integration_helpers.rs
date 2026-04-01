use std::{error::Error, rc::Rc};

use anchor_client::{
    solana_sdk::{
        account::Account,
        commitment_config::CommitmentConfig,
        instruction::Instruction,
        pubkey::Pubkey,
        signature::{read_keypair_file, Signature},
        signer::Signer,
    },
    Client, ClientError, Cluster, Program,
};

use crate::fixtures::{
    canonical_authority_keypair, canonical_server_signer_keypair, CanonicalFixtureSet,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedEd25519Message {
    pub signer_pubkey: Pubkey,
    pub message: Vec<u8>,
    pub signature: [u8; 64],
}

pub struct LocalnetRelayerHarness {
    program: Program<Rc<anchor_client::solana_sdk::signer::keypair::Keypair>>,
}

impl LocalnetRelayerHarness {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let anchor_wallet = std::env::var("ANCHOR_WALLET")?;
        let payer = Rc::new(read_keypair_file(&anchor_wallet)?);
        let client =
            Client::new_with_options(Cluster::Localnet, payer, CommitmentConfig::confirmed());
        let program = client.program(runana_program::id())?;
        Ok(Self { program })
    }

    pub fn build_initialize_request_instructions(
        &self,
        pre_instructions: &[Instruction],
    ) -> Result<Vec<Instruction>, ClientError> {
        let mut request = self.program.request();
        for ix in pre_instructions.iter().cloned() {
            request = request.instruction(ix);
        }

        request
            .accounts(runana_program::accounts::Initialize {})
            .args(runana_program::instruction::Initialize {})
            .instructions()
    }

    pub fn submit_initialize_with_pre_instructions(
        &self,
        pre_instructions: &[Instruction],
    ) -> Result<Signature, ClientError> {
        let mut request = self.program.request();
        for ix in pre_instructions.iter().cloned() {
            request = request.instruction(ix);
        }

        request
            .accounts(runana_program::accounts::Initialize {})
            .args(runana_program::instruction::Initialize {})
            .send()
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

    pub fn assert_account_missing(&self, address: Pubkey) -> Result<(), Box<dyn Error>> {
        if self.fetch_account(address)?.is_some() {
            return Err(format!("expected account {address} to be absent").into());
        }
        Ok(())
    }

    pub fn assert_accounts_missing(&self, addresses: &[Pubkey]) -> Result<(), Box<dyn Error>> {
        for address in addresses {
            self.assert_account_missing(*address)?;
        }
        Ok(())
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

fn sign_message(
    signer: &anchor_client::solana_sdk::signer::keypair::Keypair,
    message: &[u8],
) -> SignedEd25519Message {
    SignedEd25519Message {
        signer_pubkey: signer.pubkey(),
        message: message.to_vec(),
        signature: *signer.sign_message(message).as_array(),
    }
}
