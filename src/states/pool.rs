use pinocchio::pubkey::Pubkey;

pub struct PoolState{
    discriminator: u8,
    lst_mint: Pubkey,
    authority: Pubkey,
    seed: u64,
    bump: u8,
}