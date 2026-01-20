use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

#[repr(C, packed)]
pub struct PoolState {
    pub discriminator: u8,
    pub lst_mint: Pubkey,
    pub authority: Pubkey,
    pub validator_vote: Pubkey,
    pub stake_account: Pubkey,
    pub seed: u64,
    pub bump: u8,
    pub stake_bump: u8,
    pub mint_bump: u8,
    pub lst_supply: u64,
    pub pending_deposits: u64,
    pub is_initialized: bool,
    pub stake_initialized: bool,
}

use crate::Discriminator;

impl Discriminator for PoolState {
    const LEN: usize = Self::LEN;
    const DISCRIMINATOR: u8 = Self::DISCRIMINATOR;
}

impl PoolState {
    pub const LEN: usize = size_of::<Self>();
    pub const DISCRIMINATOR: u8 = 0;

    #[inline(always)]
    pub fn load_mut(bytes: &mut [u8]) -> Result<&mut Self, ProgramError> {
        if bytes.len() != PoolState::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *core::mem::transmute::<*mut u8, *mut Self>(bytes.as_mut_ptr()) })
    }

    #[inline(always)]
    pub fn load(bytes: &[u8]) -> Result<&Self, ProgramError> {
        if bytes.len() != PoolState::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(unsafe { &*core::mem::transmute::<*const u8, *const Self>(bytes.as_ptr()) })
    }

    #[inline(always)]
    pub fn discriminator(&self) -> u8 {
        self.discriminator
    }

    #[inline(always)]
    pub fn lst_mint(&self) -> Pubkey {
        self.lst_mint
    }

    #[inline(always)]
    pub fn authority(&self) -> Pubkey {
        self.authority
    }

    #[inline(always)]
    pub fn validator_vote(&self) -> Pubkey {
        self.validator_vote
    }

    #[inline(always)]
    pub fn stake_account(&self) -> Pubkey {
        self.stake_account
    }

    #[inline(always)]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    #[inline(always)]
    pub fn bump(&self) -> u8 {
        self.bump
    }

    #[inline(always)]
    pub fn stake_bump(&self) -> u8 {
        self.stake_bump
    }

    #[inline(always)]
    pub fn mint_bump(&self) -> u8 {
        self.mint_bump
    }

    pub fn lst_supply(&self) -> u64 {
        self.lst_supply
    }

    pub fn pending_deposits(&self) -> u64 {
        self.pending_deposits
    }

    #[inline(always)]
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    #[inline(always)]
    pub fn stake_initialized(&self) -> bool {
        self.stake_initialized
    }

    #[inline(always)]
    pub fn set_inner(
        &mut self,
        lst_mint: Pubkey,
        authority: Pubkey,
        validator_vote: Pubkey,
        stake_account: Pubkey,
        seed: u64,
        bump: u8,
        stake_bump: u8,
        mint_bump: u8,
        lst_supply: u64,
        is_initialized: bool,
        stake_initialized: bool,
    ) {
        self.discriminator = Self::DISCRIMINATOR;
        self.lst_mint = lst_mint;
        self.authority = authority;
        self.validator_vote = validator_vote;
        self.stake_account = stake_account;
        self.seed = seed;
        self.bump = bump;
        self.stake_bump = stake_bump;
        self.mint_bump = mint_bump;
        self.lst_supply = lst_supply;
        self.pending_deposits = 0;
        self.is_initialized = is_initialized;
        self.stake_initialized = stake_initialized;
    }
}
