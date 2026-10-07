use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};
use mpl_core::instructions::{BurnV1CpiBuilder, UpdatePluginV1CpiBuilder};
use mpl_core::types::{FreezeDelegate, Plugin};

use crate::{
    constants::{CONFIG_SEED, STAKE_SEED},
    error::StakingError,
    instructions::collection_stats::update_collection_total_staked,
    state::{StakeConfig, UserStake},
};

#[derive(Accounts)]
pub struct BurnStakedNft<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = stake_config.bump
    )]
    pub stake_config: Account<'info, StakeConfig>,

    /// CHECK: Metaplex Core Asset account to be permanently burned
    #[account(mut)]
    pub asset: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core Collection account
    #[account(mut)]
    pub collection: UncheckedAccount<'info>,

    #[account(
        mut,
        close = owner,
        seeds = [STAKE_SEED, asset.key().as_ref()],
        bump = user_stake.bump,
        has_one = owner,
        has_one = asset,
        has_one = collection
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

    /// CHECK: Metaplex Core program ID
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> BurnStakedNft<'info> {
    pub fn handler(&mut self) -> Result<()> {
        let signer_seeds: &[&[&[u8]]] = &[&[CONFIG_SEED, &[self.stake_config.bump]]];

        // 1. Compute total reward (accrued staking rewards + massive burn bonus)
        let now = Clock::get()?.unix_timestamp;
        let elapsed = now.saturating_sub(self.user_stake.last_claimed_at);
        let accrued_rewards = (elapsed as u64)
            .checked_mul(self.stake_config.points_per_sec)
            .ok_or(StakingError::NumericalOverflow)?;
        let total_rewards = accrued_rewards
            .checked_add(self.stake_config.burn_bonus_points)
            .ok_or(StakingError::NumericalOverflow)?;

        // 2. Mint reward tokens to user's ATA
        if total_rewards > 0 {
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
            token::mint_to(cpi_ctx, total_rewards)?;
        }

        // 3. Thaw the asset first so BurnDelegate can burn it
        UpdatePluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.owner.to_account_info())
            .authority(Some(&self.stake_config.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: false }))
            .invoke_signed(signer_seeds)?;

        // 4. Burn the NFT exercising BurnDelegate permission via stake_config authority
        BurnV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.owner.to_account_info())
            .authority(Some(&self.stake_config.to_account_info()))
            .system_program(Some(&self.system_program.to_account_info()))
            .invoke_signed(signer_seeds)?;

        // 4. Decrement collection-level "total_staked" attribute
        update_collection_total_staked(
            &self.collection.to_account_info(),
            &self.owner.to_account_info(),
            &self.stake_config.to_account_info(),
            &self.system_program.to_account_info(),
            &self.mpl_core_program.to_account_info(),
            false, // decrement
            signer_seeds,
        )?;

        // 5. UserStake account is closed and rent returned to owner via `close = owner`
        Ok(())
    }
}
