use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    msg,
    program_error::ProgramError,
    sysvars::{Sysvar, rent::Rent},
};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::instructions::InitializeMint2;

use super::{InitializeAccounts, InitializeData};
use crate::{PoolState, ProgramAccount, constants::*};

pub struct Initialize<'a> {
    pub accounts: InitializeAccounts<'a>,
    pub data: InitializeData,
}

impl<'a> Initialize<'a> {
    pub const DISCRIMINATOR: u8 = 0;
}

impl<'a> TryFrom<(&[u8], &'a [AccountInfo])> for Initialize<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&[u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = InitializeAccounts::try_from(accounts)?;
        let data = InitializeData::try_from(data)?;

        verify_pdas(&accounts, &data)?;

        Ok(Self { accounts, data })
    }
}

fn verify_pdas(accounts: &InitializeAccounts, data: &InitializeData) -> Result<(), ProgramError> {
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

    Ok(())
}

impl<'a> Initialize<'a> {
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

        self.create_pool_state(&pool_seeds)?;
        self.create_lst_mint(&mint_bump)?;
        self.create_stake_account(&stake_bump)?;
        self.initialize_stake()?;
        self.delegate_stake(&pool_seeds)?;

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
            0,
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

    fn create_stake_account(&self, bump: &[u8]) -> ProgramResult {
        let seeds = [
            Seed::from(b"stake"),
            Seed::from(self.accounts.pool_state.key().as_ref()),
            Seed::from(bump),
        ];
        let signer = [Signer::from(&seeds[..])];
        let rent = Rent::get()?;

        CreateAccount {
            from: self.accounts.initializer,
            to: self.accounts.stake_account,
            lamports: rent.minimum_balance(STAKE_ACCOUNT_SIZE as usize) + MIN_STAKE_DELEGATION,
            space: STAKE_ACCOUNT_SIZE,
            owner: &STAKE_PROGRAM_ID,
        }
        .invoke_signed(&signer)?;

        msg!("Stake account created");
        Ok(())
    }

    fn initialize_stake(&self) -> ProgramResult {
        let mut data = [0u8; 116];
        data[0..4].copy_from_slice(&0u32.to_le_bytes());
        data[4..36].copy_from_slice(self.accounts.pool_state.key().as_ref());
        data[36..68].copy_from_slice(self.accounts.pool_state.key().as_ref());

        let ix = Instruction {
            program_id: &STAKE_PROGRAM_ID,
            accounts: &[
                AccountMeta {
                    pubkey: self.accounts.stake_account.key(),
                    is_signer: false,
                    is_writable: true,
                },
                AccountMeta {
                    pubkey: self.accounts.rent.key(),
                    is_signer: false,
                    is_writable: false,
                },
            ],
            data: &data,
        };

        pinocchio::program::invoke(&ix, &[self.accounts.stake_account, self.accounts.rent])?;
        msg!("Stake account initialized");
        Ok(())
    }

    fn delegate_stake(&self, pool_seeds: &[Seed]) -> ProgramResult {
        let signer = [Signer::from(pool_seeds)];
        let data = 2u32.to_le_bytes();

        let ix = Instruction {
            program_id: &STAKE_PROGRAM_ID,
            accounts: &[
                AccountMeta {
                    pubkey: self.accounts.stake_account.key(),
                    is_signer: false,
                    is_writable: true,
                },
                AccountMeta {
                    pubkey: self.accounts.validator_vote.key(),
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: self.accounts.clock.key(),
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: self.accounts.stake_history.key(),
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: self.accounts.stake_config.key(),
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta {
                    pubkey: self.accounts.pool_state.key(),
                    is_signer: true,
                    is_writable: false,
                },
            ],
            data: &data,
        };

        pinocchio::program::invoke_signed(
            &ix,
            &[
                self.accounts.stake_account,
                self.accounts.validator_vote,
                self.accounts.clock,
                self.accounts.stake_history,
                self.accounts.stake_config,
                self.accounts.pool_state,
            ],
            &signer,
        )?;

        msg!("Stake delegated");
        Ok(())
    }
}
