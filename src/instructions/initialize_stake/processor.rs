use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    sysvars::{Sysvar, rent::Rent},
};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::instructions::InitializeMint2;

use super::{InitializeAccounts, InitializeData};
use crate::{
    PoolState, ProgramAccount, constants::*, create_stake_account, delegate_stake, initialize_stake,
};

pub struct Initialize<'a> {
    pub accounts: InitializeAccounts<'a>,
    pub data: InitializeData,
}

impl<'a> TryFrom<(&[u8], &'a [AccountInfo])> for Initialize<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&[u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = InitializeAccounts::try_from(accounts)?;
        let data = InitializeData::try_from(data)?;

        let seed_bytes = data.seed.to_le_bytes();

        ProgramAccount::verify(
            &[
                Seed::from(b"lst_pool"),
                Seed::from(accounts.initializer.key().as_ref()),
                Seed::from(&seed_bytes),
            ],
            accounts.pool_state,
            data.pool_bump,
        )?;

        ProgramAccount::verify(
            &[
                Seed::from(b"lst_mint"),
                Seed::from(accounts.pool_state.key().as_ref()),
            ],
            accounts.lst_mint,
            data.mint_bump,
        )?;

        ProgramAccount::verify(
            &[
                Seed::from(b"stake"),
                Seed::from(accounts.pool_state.key().as_ref()),
            ],
            accounts.stake_account,
            data.stake_bump,
        )?;

        Ok(Self { accounts, data })
    }
}

impl<'a> Initialize<'a> {
    pub const DISCRIMINATOR: u8 = 0;

    pub fn process(&self) -> ProgramResult {
        let seed_bytes = self.data.seed.to_le_bytes();
        let pool_bump = [self.data.pool_bump];
        let mint_bump = [self.data.mint_bump];
        let stake_bump = [self.data.stake_bump];

        let pool_seeds = [
            Seed::from(b"lst_pool"),
            Seed::from(self.accounts.initializer.key().as_ref()),
            Seed::from(&seed_bytes),
            Seed::from(&pool_bump),
        ];

        let stake_seeds = [
            Seed::from(b"stake"),
            Seed::from(self.accounts.pool_state.key().as_ref()),
            Seed::from(&stake_bump),
        ];

        self.create_pool_state(&pool_seeds)?;
        self.create_lst_mint(&mint_bump)?;

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
            *self.accounts.lst_mint.key(),
            *self.accounts.initializer.key(),
            *self.accounts.validator_vote.key(),
            *self.accounts.stake_account.key(),
            self.data.seed,
            self.data.pool_bump,
            self.data.stake_bump,
            self.data.mint_bump,
            0,
            true,
            true,
        );

        msg!("Pool state initialized");
        Ok(())
    }

    fn create_lst_mint(&self, bump: &[u8]) -> ProgramResult {
        let seeds = [
            Seed::from(b"lst_mint"),
            Seed::from(self.accounts.pool_state.key().as_ref()),
            Seed::from(bump),
        ];
        let signer = [Signer::from(&seeds[..])];
        let rent = Rent::get()?;

        CreateAccount {
            from: self.accounts.initializer,
            to: self.accounts.lst_mint,
            lamports: rent.minimum_balance(pinocchio_token::state::Mint::LEN),
            space: pinocchio_token::state::Mint::LEN as u64,
            owner: &pinocchio_token::ID,
        }
        .invoke_signed(&signer)?;

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
