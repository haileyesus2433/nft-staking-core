use anchor_lang::prelude::*;
use mpl_core::instructions::AddCollectionPluginV1CpiBuilder;
use mpl_core::types::{Attribute, Attributes, Plugin, PluginAuthority};

use crate::{
    constants::{CONFIG_SEED, TOTAL_STAKED_KEY},
    state::StakeConfig,
};

#[derive(Accounts)]
pub struct InitCollectionStats<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [CONFIG_SEED],
        bump = stake_config.bump,
        has_one = admin
    )]
    pub stake_config: Account<'info, StakeConfig>,

    /// CHECK: Metaplex Core Collection account
    #[account(mut)]
    pub collection: UncheckedAccount<'info>,

    /// CHECK: Metaplex Core program ID
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> InitCollectionStats<'info> {
    pub fn handler(&mut self) -> Result<()> {
        let attributes = Attributes {
            attribute_list: vec![Attribute {
                key: TOTAL_STAKED_KEY.to_string(),
                value: "0".to_string(),
            }],
        };

        AddCollectionPluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
            .collection(&self.collection.to_account_info())
            .payer(&self.admin.to_account_info())
            .authority(Some(&self.admin.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .plugin(Plugin::Attributes(attributes))
            .init_authority(PluginAuthority::Address {
                address: self.stake_config.key(),
            })
            .invoke()?;

        Ok(())
    }
}
