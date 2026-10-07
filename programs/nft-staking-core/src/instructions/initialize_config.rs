use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

use crate::{constants::CONFIG_SEED, state::StakeConfig};

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + StakeConfig::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub stake_config: Account<'info, StakeConfig>,

    pub reward_mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
}

impl<'info> InitializeConfig<'info> {
    pub fn handler(&mut self, points_per_sec: u64, burn_bonus_points: u64, bump: u8) -> Result<()> {
        self.stake_config.set_inner(StakeConfig {
            admin: self.admin.key(),
            points_per_sec,
            burn_bonus_points,
            reward_mint: self.reward_mint.key(),
            bump,
        });
        Ok(())
    }
}
