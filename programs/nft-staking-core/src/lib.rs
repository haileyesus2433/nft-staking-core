pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use error::*;
pub use instructions::*;
pub use state::*;

declare_id!("HFpN1kKDxGdS8XG9NAP63gA4dkzMvrE8hofP9Xi1NgPm");

#[program]
pub mod nft_staking_core {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        points_per_sec: u64,
        burn_bonus_points: u64,
    ) -> Result<()> {
        let bump = ctx.bumps.stake_config;
        ctx.accounts.handler(points_per_sec, burn_bonus_points, bump)
    }

    pub fn init_collection_stats(ctx: Context<InitCollectionStats>) -> Result<()> {
        ctx.accounts.handler()
    }

    pub fn stake(ctx: Context<Stake>) -> Result<()> {
        let bump = ctx.bumps.user_stake;
        ctx.accounts.handler(bump)
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        ctx.accounts.handler()
    }

    pub fn burn_staked_nft(ctx: Context<BurnStakedNft>) -> Result<()> {
        ctx.accounts.handler()
    }

    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        ctx.accounts.handler()
    }
}
