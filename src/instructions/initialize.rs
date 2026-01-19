use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{Sysvar, rent::Rent},
};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::instructions::InitializeMint2;

use crate::{PinocchioError, PoolState, ProgramAccount, SignerAccount};

const VOTE_PROGRAM_ID: [u8; 32] = [
    7, 97, 72, 29, 53, 116, 116, 187, 124, 77, 118, 36, 235, 211, 189, 179, 216, 53, 94, 115, 209,
    16, 67, 252, 13, 163, 83, 128, 0, 0, 0, 0,
]; // Vote111111111111111111111111111111111111111

pub struct InitializeAccounts<'a> {
    pub initializer: &'a AccountInfo,
    pub pool_state: &'a AccountInfo,
    pub lst_mint: &'a AccountInfo,
    pub validator_vote: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for InitializeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            initializer,
            pool_state,
            lst_mint,
            validator_vote,
            system_program,
            token_program,
        ] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(initializer)?;
        ProgramAccount::check_system_program(system_program)?;
        ProgramAccount::check_token_program(token_program)?;

        if validator_vote.owner().as_ref() != VOTE_PROGRAM_ID {
            msg!("Invalid validator vote account");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self {
            initializer,
            pool_state,
            lst_mint,
            validator_vote,
            system_program,
            token_program,
        })
    }
}

#[repr(C, packed)]
pub struct InitializeInstructionData {
    pub seed: u64,
    pub pool_bump: u8,
    pub mint_bump: u8,
}

impl<'a> TryFrom<&'a [u8]> for InitializeInstructionData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != core::mem::size_of::<InitializeInstructionData>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let seed = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let pool_bump = data[8];
        let mint_bump = data[9];

        if seed == 0 {
            return Err(PinocchioError::InvalidSeed.into());
        }

        Ok(Self {
            seed,
            pool_bump,
            mint_bump,
        })
    }
}

pub struct Initialize<'a> {
    pub accounts: InitializeAccounts<'a>,
    pub instruction_data: InitializeInstructionData,
}

impl<'a> TryFrom<(&'a [AccountInfo], &'a [u8])> for Initialize<'a> {
    type Error = ProgramError;

    fn try_from((accounts, data): (&'a [AccountInfo], &'a [u8])) -> Result<Self, Self::Error> {
        let accounts = InitializeAccounts::try_from(accounts)?;
        let instruction_data = InitializeInstructionData::try_from(data)?;

        let seed_bytes = instruction_data.seed.to_le_bytes();

        ProgramAccount::verify(
            &[
                Seed::from(b"lst_pool"),
                Seed::from(accounts.initializer.key().as_ref()),
                Seed::from(&seed_bytes),
            ],
            accounts.pool_state,
            instruction_data.pool_bump,
        )?;

        ProgramAccount::verify(
            &[
                Seed::from(b"lst_mint"),
                Seed::from(accounts.pool_state.key().as_ref()),
            ],
            accounts.lst_mint,
            instruction_data.mint_bump,
        )?;

        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}

impl<'a> Initialize<'a> {
    pub const DISCRIMINATOR: u8 = 0;

    pub fn process(&self) -> Result<(), ProgramError> {
        let seed_bytes = self.instruction_data.seed.to_le_bytes();
        let pool_bump_byte = [self.instruction_data.pool_bump];
        let mint_bump_byte = [self.instruction_data.mint_bump];

        let pool_state_seeds = [
            Seed::from(b"lst_pool"),
            Seed::from(self.accounts.initializer.key().as_ref()),
            Seed::from(&seed_bytes),
            Seed::from(&pool_bump_byte),
        ];

        ProgramAccount::init::<PoolState>(
            self.accounts.initializer,
            self.accounts.pool_state,
            &pool_state_seeds,
            PoolState::LEN,
        )?;

        {
            let mut pool_state_data = self.accounts.pool_state.try_borrow_mut_data()?;
            let pool_state = PoolState::load_mut(&mut pool_state_data)?;

            pool_state.set_inner(
                *self.accounts.lst_mint.key(),
                *self.accounts.initializer.key(),
                *self.accounts.validator_vote.key(),
                Pubkey::default(),
                self.instruction_data.seed,
                self.instruction_data.pool_bump,
                0,
                0,
                0,
                true,
                false,
            );
        }

        msg!("Pool state initialized");

        let lst_mint_seeds = [
            Seed::from(b"lst_mint"),
            Seed::from(self.accounts.pool_state.key().as_ref()),
            Seed::from(&mint_bump_byte),
        ];
        let mint_signer = [Signer::from(&lst_mint_seeds)];

        let lamports = Rent::get()?.minimum_balance(pinocchio_token::state::Mint::LEN);

        CreateAccount {
            from: self.accounts.initializer,
            to: self.accounts.lst_mint,
            lamports,
            space: pinocchio_token::state::Mint::LEN as u64,
            owner: &pinocchio_token::ID,
        }
        .invoke_signed(&mint_signer)?;

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
