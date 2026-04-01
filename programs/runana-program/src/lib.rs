use anchor_lang::{
    prelude::*,
    solana_program::sysvar::instructions::{
        load_current_index_checked, load_instruction_at_checked,
    },
};
use solana_program::{ed25519_program, hash::hashv};

declare_id!("CaUejpPZoNjFmSrkfbazrjBUXE8FK1c2Hoz64NFsTfLm");

#[program]
pub mod runana_program {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }

    pub fn apply_battle_settlement_batch_v1(
        ctx: Context<ApplyBattleSettlementBatchV1>,
        args: ApplyBattleSettlementBatchV1Args,
    ) -> Result<()> {
        verify_ed25519_preinstructions(&ctx)?;
        verify_nonce_range(&args.payload)?;
        verify_histogram_count(&args.payload)?;
        verify_batch_hash(&args.payload)?;

        msg!(
            "settlement_smoke_ok batch_id={} battle_count={}",
            args.payload.batch_id,
            args.payload.battle_count
        );

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

#[derive(Accounts)]
pub struct ApplyBattleSettlementBatchV1<'info> {
    /// CHECK: settlement permit subject only; ownership binding is deferred until character root exists
    pub player_authority: UncheckedAccount<'info>,
    /// CHECK: sysvar instructions account is validated by address and parsed at runtime
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct ApplyBattleSettlementBatchV1Args {
    pub payload: SettlementBatchPayloadV1,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct SettlementBatchPayloadV1 {
    pub character_id: [u8; 16],
    pub batch_id: u64,
    pub start_nonce: u64,
    pub end_nonce: u64,
    pub battle_count: u16,
    pub start_state_hash: [u8; 32],
    pub end_state_hash: [u8; 32],
    pub zone_progress_delta: Vec<ZoneProgressDeltaEntry>,
    pub encounter_histogram: Vec<EncounterCountEntry>,
    pub optional_loadout_revision: Option<u32>,
    pub batch_hash: [u8; 32],
    pub first_battle_ts: u64,
    pub last_battle_ts: u64,
    pub season_id: u32,
    pub schema_version: u16,
    pub signature_scheme: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct EncounterCountEntry {
    pub zone_id: u16,
    pub enemy_archetype_id: u16,
    pub count: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct ZoneProgressDeltaEntry {
    pub zone_id: u16,
    pub new_state: u8,
}

#[derive(AnchorSerialize)]
struct SettlementBatchPayloadPreimageV1 {
    character_id: [u8; 16],
    batch_id: u64,
    start_nonce: u64,
    end_nonce: u64,
    battle_count: u16,
    first_battle_ts: u64,
    last_battle_ts: u64,
    season_id: u32,
    start_state_hash: [u8; 32],
    end_state_hash: [u8; 32],
    zone_progress_delta: Vec<ZoneProgressDeltaEntry>,
    encounter_histogram: Vec<EncounterCountEntry>,
    optional_loadout_revision: Option<u32>,
    schema_version: u16,
    signature_scheme: u8,
}

impl From<&SettlementBatchPayloadV1> for SettlementBatchPayloadPreimageV1 {
    fn from(payload: &SettlementBatchPayloadV1) -> Self {
        Self {
            character_id: payload.character_id,
            batch_id: payload.batch_id,
            start_nonce: payload.start_nonce,
            end_nonce: payload.end_nonce,
            battle_count: payload.battle_count,
            first_battle_ts: payload.first_battle_ts,
            last_battle_ts: payload.last_battle_ts,
            season_id: payload.season_id,
            start_state_hash: payload.start_state_hash,
            end_state_hash: payload.end_state_hash,
            zone_progress_delta: payload.zone_progress_delta.clone(),
            encounter_histogram: payload.encounter_histogram.clone(),
            optional_loadout_revision: payload.optional_loadout_revision,
            schema_version: payload.schema_version,
            signature_scheme: payload.signature_scheme,
        }
    }
}

fn verify_ed25519_preinstructions(ctx: &Context<ApplyBattleSettlementBatchV1>) -> Result<()> {
    let instructions_sysvar = ctx.accounts.instructions_sysvar.to_account_info();
    let current_index = load_current_index_checked(&instructions_sysvar)
        .map_err(|_| error!(SettlementError::InvalidInstructionsSysvar))?
        as usize;

    require!(
        current_index >= 2,
        SettlementError::MissingEd25519Preinstructions
    );

    let first_ix = load_instruction_at_checked(current_index - 2, &instructions_sysvar)
        .map_err(|_| error!(SettlementError::InvalidInstructionsSysvar))?;
    let second_ix = load_instruction_at_checked(current_index - 1, &instructions_sysvar)
        .map_err(|_| error!(SettlementError::InvalidInstructionsSysvar))?;

    require_keys_eq!(
        first_ix.program_id,
        ed25519_program::ID,
        SettlementError::InvalidEd25519InstructionOrder
    );
    require_keys_eq!(
        second_ix.program_id,
        ed25519_program::ID,
        SettlementError::InvalidEd25519InstructionOrder
    );

    Ok(())
}

fn verify_nonce_range(payload: &SettlementBatchPayloadV1) -> Result<()> {
    let expected_battle_count = payload
        .end_nonce
        .checked_sub(payload.start_nonce)
        .and_then(|delta| delta.checked_add(1))
        .ok_or_else(|| error!(SettlementError::InvalidNonceRange))?;

    require!(
        expected_battle_count == u64::from(payload.battle_count),
        SettlementError::InvalidNonceRange
    );

    Ok(())
}

fn verify_histogram_count(payload: &SettlementBatchPayloadV1) -> Result<()> {
    let histogram_total: u64 = payload
        .encounter_histogram
        .iter()
        .map(|entry| u64::from(entry.count))
        .sum();

    require!(
        histogram_total == u64::from(payload.battle_count),
        SettlementError::HistogramCountMismatch
    );

    Ok(())
}

fn verify_batch_hash(payload: &SettlementBatchPayloadV1) -> Result<()> {
    let preimage = SettlementBatchPayloadPreimageV1::from(payload);
    let preimage_bytes = preimage
        .try_to_vec()
        .map_err(|_| error!(SettlementError::PreimageSerializationFailed))?;
    let recomputed = hashv(&[&preimage_bytes]).to_bytes();

    require!(
        recomputed == payload.batch_hash,
        SettlementError::BatchHashMismatch
    );

    Ok(())
}

#[error_code]
pub enum SettlementError {
    #[msg("The instructions sysvar account could not be parsed")]
    InvalidInstructionsSysvar,
    #[msg("Two ed25519 verification instructions must precede the settlement instruction")]
    MissingEd25519Preinstructions,
    #[msg("The settlement instruction must be preceded by two ed25519 instructions in order")]
    InvalidEd25519InstructionOrder,
    #[msg("The settlement nonce range does not match battle_count")]
    InvalidNonceRange,
    #[msg("The encounter histogram total does not match battle_count")]
    HistogramCountMismatch,
    #[msg("The canonical settlement preimage could not be serialized")]
    PreimageSerializationFailed,
    #[msg("The recomputed batch hash does not match the payload batch hash")]
    BatchHashMismatch,
}
