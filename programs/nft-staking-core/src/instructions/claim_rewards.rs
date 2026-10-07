use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};
use mpl_core::types::{FreezeDelegate, PluginType};

use crate::{
    constants::{CONFIG_SEED, STAKE_SEED},
    error::StakingError,
    state::{StakeConfig, UserStake},
};

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = stake_config.bump
    )]
    pub stake_config: Account<'info, StakeConfig>,

    /// CHECK: Metaplex Core Asset account verified against UserStake
    pub asset: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [STAKE_SEED, asset.key().as_ref()],
        bump = user_stake.bump,
        has_one = owner,
        has_one = asset
    )]
    pub user_stake: Account<'info, UserStake>,

    #[account(
        mut,
        address = stake_config.reward_mint
    )]
    pub reward_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = reward_mint,
        associated_token::authority = owner
    )]
    pub user_reward_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

impl<'info> ClaimRewards<'info> {
    pub fn handler(&mut self) -> Result<()> {
        // 1. Verify the NFT is currently staked and frozen
        let (_, freeze_delegate, _) =
            mpl_core::fetch_asset_plugin::<FreezeDelegate>(&self.asset, PluginType::FreezeDelegate)
                .map_err(|_| StakingError::AssetNotFrozen)?;

        require!(freeze_delegate.frozen, StakingError::AssetNotFrozen);

        // 2. Compute accumulated rewards
        let now = Clock::get()?.unix_timestamp;
        let elapsed = now.saturating_sub(self.user_stake.last_claimed_at);
        let rewards = (elapsed as u64)
            .checked_mul(self.stake_config.points_per_sec)
            .ok_or(StakingError::NumericalOverflow)?;

        require!(rewards > 0, StakingError::NoRewardsAvailable);

        // 3. Mint reward tokens to the user's ATA
        let signer_seeds: &[&[&[u8]]] = &[&[CONFIG_SEED, &[self.stake_config.bump]]];
        let cpi_accounts = MintTo {
            mint: self.reward_mint.to_account_info(),
            to: self.user_reward_ata.to_account_info(),
            authority: self.stake_config.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            self.token_program.key(),
            cpi_accounts,
            signer_seeds,
        );
        token::mint_to(cpi_ctx, rewards)?;

        // 4. Update last claimed timestamp; keep NFT staked and frozen
        self.user_stake.last_claimed_at = now;

        Ok(())
    }
}
