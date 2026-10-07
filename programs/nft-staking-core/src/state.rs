use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct StakeConfig {
    pub admin: Pubkey,
    pub points_per_sec: u64,
    pub burn_bonus_points: u64,
    pub reward_mint: Pubkey,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct UserStake {
    pub owner: Pubkey,
    pub asset: Pubkey,
    pub collection: Pubkey,
    pub staked_at: i64,
    pub last_claimed_at: i64,
    pub bump: u8,
}
