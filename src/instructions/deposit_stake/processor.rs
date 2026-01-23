use pinocchio::{
    ProgramResult, account_info::AccountInfo, instruction::Seed, program_error::ProgramError,
};
use pinocchio_system::instructions::Transfer;
use pinocchio_token::instructions::MintTo;

use crate::{DepositAccounts, DepositInstructionData, PoolState, ProgramAccount};

pub struct Deposit<'a> {
    pub accounts: DepositAccounts<'a>,
    pub instruction_data: DepositInstructionData,
}

impl<'a> TryFrom<(&[u8], &'a [AccountInfo])> for Deposit<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&[u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = DepositAccounts::try_from(accounts)?;
        let instruction_data = DepositInstructionData::try_from(data)?;

        let pool_state_data = accounts.pool_state.try_borrow_data()?;
        let pool_state = PoolState::load(&pool_state_data)?;

        let seed_bytes = pool_state.seed.to_le_bytes();
        ProgramAccount::verify(
            &[
                Seed::from(b"lst_pool"),
                Seed::from(pool_state.authority.as_ref()),
                Seed::from(&seed_bytes),
            ],
            accounts.pool_state,
            pool_state.bump,
        )?;

        if pool_state.is_initialized == false {
            return Err(ProgramError::UninitializedAccount);
        }

        if accounts.pool_stake.key() != &pool_state.stake_account {
            return Err(ProgramError::InvalidAccountData);
        }

        if accounts.reserve_stake.key() != &pool_state.reserve_stake {
            return Err(ProgramError::InvalidAccountData);
        }

        if accounts.lst_mint.key() != &pool_state.lst_mint {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}

impl<'a> Deposit<'a> {
    pub const DISCRIMINATOR: u8 = 1;

    pub fn process(&self) -> ProgramResult {
        let lst_amount: u64 = {
            let pool_state_data = self.accounts.pool_state.try_borrow_data()?;
            let pool_state = PoolState::load(&pool_state_data)?;

            let lst_amount = self.calculate_lst_amount(&pool_state)?;

            let seed_binding = pool_state.seed.to_le_bytes();
            let bump_binding = [pool_state.bump];
            let pool_seeds = [
                Seed::from(b"lst_pool"),
                Seed::from(pool_state.authority.as_ref()),
                Seed::from(&seed_binding),
                Seed::from(&bump_binding),
            ];

            let _ = pool_state;

            Transfer {
                from: self.accounts.depositor,
                to: self.accounts.reserve_stake,
                lamports: self.instruction_data.amount,
            }
            .invoke()?;

            self.mint_lst(lst_amount, &pool_seeds)?;

            lst_amount
        };

        let mut pool_data = self.accounts.pool_state.try_borrow_mut_data()?;
        let pool = PoolState::load_mut(&mut pool_data)?;
        pool.lst_supply = pool
            .lst_supply
            .checked_add(lst_amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        Ok(())
    }

    fn calculate_lst_amount(&self, pool: &PoolState) -> Result<u64, ProgramError> {
        let pool_stake_sol = self.accounts.pool_stake.lamports();
        let reserve_stake_sol = self.accounts.reserve_stake.lamports();

        let total_pool_value = pool_stake_sol
            .checked_add(reserve_stake_sol)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        if pool.lst_supply == 0 {
            Ok(self.instruction_data.amount)
        } else {
            let lst_amount = (self.instruction_data.amount as u128)
                .checked_mul(pool.lst_supply as u128)
                .ok_or(ProgramError::ArithmeticOverflow)?
                .checked_div(total_pool_value as u128)
                .ok_or(ProgramError::ArithmeticOverflow)?;

            Ok(lst_amount as u64)
        }
    }

    fn mint_lst(&self, amount: u64, pool_seeds: &[Seed]) -> ProgramResult {
        let signer = [pinocchio::instruction::Signer::from(pool_seeds)];

        MintTo {
            mint: self.accounts.lst_mint,
            account: self.accounts.depositor_lst_ata,
            mint_authority: self.accounts.pool_state,
            amount,
        }
        .invoke_signed(&signer)?;

        Ok(())
    }
}
