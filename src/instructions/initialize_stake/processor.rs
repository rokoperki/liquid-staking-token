use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    pubkey::find_program_address,
    sysvars::{Sysvar, rent::Rent},
};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::instructions::{InitializeMint2, MintTo};

use super::{InitializeAccounts, InitializeData};
use crate::{
    AssociatedToken, PoolState, ProgramAccount, constants::*, create_stake_account, delegate_stake,
    initialize_stake,
};

pub struct Initialize<'a> {
    pub accounts: InitializeAccounts<'a>,
    pub data: InitializeData,
    pub pool_bump: u8,
    pub stake_bump: u8,
    pub reserve_bump: u8,
}

impl<'a> TryFrom<(&[u8], &'a [AccountInfo])> for Initialize<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&[u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = InitializeAccounts::try_from(accounts)?;
        let data = InitializeData::try_from(data)?;

        let seed_bytes = data.seed.to_le_bytes();

        let (pool_pda, pool_bump) = find_program_address(&[b"lst_pool", &seed_bytes], &crate::ID);
        if accounts.pool_state.key() != &pool_pda {
            return Err(ProgramError::InvalidSeeds);
        }

        let (stake_pda, stake_bump) =
            find_program_address(&[b"stake", accounts.pool_state.key().as_ref()], &crate::ID);
        if accounts.stake_account.key() != &stake_pda {
            return Err(ProgramError::InvalidSeeds);
        }

        let (reserve_pda, reserve_bump) = find_program_address(
            &[b"reserve_stake", accounts.pool_state.key().as_ref()],
            &crate::ID,
        );
        if accounts.reserve_stake.key() != &reserve_pda {
            return Err(ProgramError::InvalidSeeds);
        }

        Ok(Self {
            accounts,
            data,
            pool_bump,
            stake_bump,
            reserve_bump,
        })
    }
}

impl<'a> Initialize<'a> {
    pub const DISCRIMINATOR: u8 = 0;

    pub fn process(&self) -> ProgramResult {
        let seed_bytes = self.data.seed.to_le_bytes();
        let pool_bump = [self.pool_bump];
        let stake_bump = [self.stake_bump];
        let reserve_bump = [self.reserve_bump];

        let pool_seeds = [
            Seed::from(b"lst_pool"),
            Seed::from(&seed_bytes),
            Seed::from(&pool_bump),
        ];

        let stake_seeds = [
            Seed::from(b"stake"),
            Seed::from(self.accounts.pool_state.key().as_ref()),
            Seed::from(&stake_bump),
        ];

        let reserve_stake_seeds = [
            Seed::from(b"reserve_stake"),
            Seed::from(self.accounts.pool_state.key().as_ref()),
            Seed::from(&reserve_bump),
        ];

        self.create_pool_state(&pool_seeds)?;
        self.create_lst_mint()?;

        AssociatedToken::init(
            self.accounts.initializer_lst_ata,
            self.accounts.lst_mint,
            self.accounts.initializer,
            self.accounts.initializer,
            self.accounts.system_program,
            self.accounts.token_program,
        )?;

        create_stake_account(
            self.accounts.initializer,
            self.accounts.reserve_stake,
            0,
            &reserve_stake_seeds,
        )?;

        create_stake_account(
            self.accounts.initializer,
            self.accounts.stake_account,
            MIN_STAKE_DELEGATION,
            &stake_seeds,
        )?;

        initialize_stake(
            self.accounts.stake_account,
            self.accounts.rent,
            self.accounts.pool_state,
            self.accounts.pool_state,
        )?;

        delegate_stake(
            self.accounts.stake_account,
            self.accounts.validator_vote,
            self.accounts.clock,
            self.accounts.stake_history,
            self.accounts.stake_config,
            self.accounts.pool_state,
            &pool_seeds,
        )?;

        self.mint_initial_lst(&pool_seeds)?;

        Ok(())
    }

    fn mint_initial_lst(&self, pool_seeds: &[Seed]) -> ProgramResult {
        let signer = [Signer::from(pool_seeds)];

        MintTo {
            mint: self.accounts.lst_mint,
            account: self.accounts.initializer_lst_ata,
            mint_authority: self.accounts.pool_state,
            amount: MIN_STAKE_DELEGATION,
        }
        .invoke_signed(&signer)?;

        Ok(())
    }

    fn create_pool_state(&self, seeds: &[Seed]) -> ProgramResult {
        ProgramAccount::init::<PoolState>(
            self.accounts.initializer,
            self.accounts.pool_state,
            seeds,
            PoolState::LEN,
        )?;

        let mut data = self.accounts.pool_state.try_borrow_mut_data()?;
        let pool = PoolState::load_mut(&mut data)?;
        pool.set_inner(
            1,
            *self.accounts.lst_mint.key(),
            *self.accounts.initializer.key(),
            *self.accounts.validator_vote.key(),
            *self.accounts.stake_account.key(),
            *self.accounts.reserve_stake.key(),
            self.data.seed,
            self.pool_bump,
            self.stake_bump,
            self.reserve_bump,
            MIN_STAKE_DELEGATION,
        );

        msg!("Pool state initialized");
        Ok(())
    }

    fn create_lst_mint(&self) -> ProgramResult {
        let rent = Rent::get()?;

        CreateAccount {
            from: self.accounts.initializer,
            to: self.accounts.lst_mint,
            lamports: rent.minimum_balance(pinocchio_token::state::Mint::LEN),
            space: pinocchio_token::state::Mint::LEN as u64,
            owner: &pinocchio_token::ID,
        }
        .invoke()?;

        InitializeMint2 {
            mint: self.accounts.lst_mint,
            decimals: 9,
            mint_authority: self.accounts.pool_state.key(),
            freeze_authority: None,
        }
        .invoke()?;

        msg!("LST mint initialized");
        Ok(())
    }
}
