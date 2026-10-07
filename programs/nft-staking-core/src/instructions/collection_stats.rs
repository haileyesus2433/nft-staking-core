use anchor_lang::prelude::*;
use mpl_core::types::{Attribute, Attributes, Plugin, PluginAuthority, PluginType};

use crate::{constants::TOTAL_STAKED_KEY, error::StakingError};

pub fn update_collection_total_staked<'a>(
    collection: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    mpl_core_program: &AccountInfo<'a>,
    increment: bool,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    match mpl_core::fetch_collection_plugin::<Attributes>(collection, PluginType::Attributes) {
        Ok((_plugin_auth, mut attributes, _offset)) => {
            let mut found = false;
            for attr in attributes.attribute_list.iter_mut() {
                if attr.key == TOTAL_STAKED_KEY {
                    let count: u64 = attr
                        .value
                        .parse()
                        .map_err(|_| StakingError::InvalidStakingCount)?;
                    let new_count = if increment {
                        count
                            .checked_add(1)
                            .ok_or(StakingError::NumericalOverflow)?
                    } else {
                        count.saturating_sub(1)
                    };
                    attr.value = new_count.to_string();
                    found = true;
                    break;
                }
            }

            if !found {
                attributes.attribute_list.push(Attribute {
                    key: TOTAL_STAKED_KEY.to_string(),
                    value: if increment {
                        "1".to_string()
                    } else {
                        "0".to_string()
                    },
                });
            }

            mpl_core::instructions::UpdateCollectionPluginV1CpiBuilder::new(mpl_core_program)
                .collection(collection)
                .payer(payer)
                .authority(Some(authority))
                .system_program(system_program)
                .plugin(Plugin::Attributes(attributes))
                .invoke_signed(signer_seeds)?;
        }
        Err(_) => {
            let attributes = Attributes {
                attribute_list: vec![Attribute {
                    key: TOTAL_STAKED_KEY.to_string(),
                    value: if increment {
                        "1".to_string()
                    } else {
                        "0".to_string()
                    },
                }],
            };

            mpl_core::instructions::AddCollectionPluginV1CpiBuilder::new(mpl_core_program)
                .collection(collection)
                .payer(payer)
                .authority(Some(authority))
                .system_program(system_program)
                .plugin(Plugin::Attributes(attributes))
                .init_authority(PluginAuthority::Address {
                    address: authority.key(),
                })
                .invoke_signed(signer_seeds)?;
        }
    }

    Ok(())
}
