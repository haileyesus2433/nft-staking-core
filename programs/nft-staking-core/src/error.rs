use anchor_lang::prelude::*;

#[error_code]
pub enum StakingError {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Invalid collection provided")]
    InvalidCollection,
    #[msg("Asset is not frozen")]
    AssetNotFrozen,
    #[msg("Asset is still frozen")]
    AssetStillFrozen,
    #[msg("Failed to parse staking count")]
    InvalidStakingCount,
    #[msg("No rewards available to claim")]
    NoRewardsAvailable,
    #[msg("Numerical overflow")]
    NumericalOverflow,
}
