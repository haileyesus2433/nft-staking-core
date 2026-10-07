use anchor_lang::prelude::*;
use mpl_core::instructions::AddPluginV1CpiBuilder;
use mpl_core::types::{BurnDelegate, FreezeDelegate, Plugin, PluginAuthority};

use crate::{
    constants::{CONFIG_SEED, STAKE_SEED},
    instructions::collection_stats::update_collection_total_staked,
    state::{StakeConfig, UserStake},
};

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = stake_config.bump
    )]
    pub stake_config: Account<'info, StakeConfig>,

    /// CHECK: Metaplex Core Asset account to stake
    #[account(mut)]
    pub asset: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core Collection account
    #[account(mut)]
    pub collection: UncheckedAccount<'info>,

    #[account(
        init,
        payer = owner,
        space = 8 + UserStake::INIT_SPACE,
        seeds = [STAKE_SEED, asset.key().as_ref()],
        bump
    )]
    pub user_stake: Account<'info, UserStake>,

    /// CHECK: Metaplex Core program ID
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> Stake<'info> {
    pub fn handler(&mut self, bump: u8) -> Result<()> {
        // 1. Add FreezeDelegate plugin to asset and freeze it
        AddPluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.owner.to_account_info())
            .authority(Some(&self.owner.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
            .init_authority(PluginAuthority::Address {
                address: self.stake_config.key(),
            })
            .invoke()?;

        // 2. Add BurnDelegate plugin to asset with stake_config authority
        AddPluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.owner.to_account_info())
            .authority(Some(&self.owner.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .plugin(Plugin::BurnDelegate(BurnDelegate {}))
            .init_authority(PluginAuthority::Address {
                address: self.stake_config.key(),
            })
            .invoke()?;

        // 3. Increment collection-level "total_staked" attribute
        let signer_seeds: &[&[&[u8]]] = &[&[CONFIG_SEED, &[self.stake_config.bump]]];
        update_collection_total_staked(
            &self.collection.to_account_info(),
            &self.owner.to_account_info(),
            &self.stake_config.to_account_info(),
            &self.system_program.to_account_info(),
            &self.mpl_core_program.to_account_info(),
            true, // increment
            signer_seeds,
        )?;

        // 4. Initialize UserStake account
        let now = Clock::get()?.unix_timestamp;
        self.user_stake.set_inner(UserStake {
            owner: self.owner.key(),
            asset: self.asset.key(),
            collection: self.collection.key(),
            staked_at: now,
            last_claimed_at: now,
            bump,
        });

        Ok(())
    }
}
