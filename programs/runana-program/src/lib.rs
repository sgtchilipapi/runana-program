use anchor_lang::prelude::*;

declare_id!("CaUejpPZoNjFmSrkfbazrjBUXE8FK1c2Hoz64NFsTfLm");

#[program]
pub mod runana_program {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
