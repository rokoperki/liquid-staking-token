use pinocchio::{
    account_info::AccountInfo, instruction::Seed, msg, program_error::ProgramError, sysvars::{Sysvar, rent::Rent}
};
use pinocchio_token::instructions::Burn;

use crate::{
    MIN_STAKE_DELEGATION, PoolState, ProgramAccount, STAKE_ACCOUNT_SIZE, WithdrawAccounts,
    WithdrawInstructionData, create_stake_account, deactivate_stake, split_stake,
};

pub struct Withdraw<'a> {
    pub accounts: WithdrawAccounts<'a>,
    pub instruction_data: WithdrawInstructionData,
}

impl<'a> TryFrom<(&[u8], &'a [AccountInfo])> for Withdraw<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&[u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = WithdrawAccounts::try_from(accounts)?;
        let instruction_data = WithdrawInstructionData::try_from(data)?;

        let pool_state_data = accounts.pool_state.try_borrow_data()?;
        let pool_state = PoolState::load(&pool_state_data)?;

        if pool_state.is_initialized == false {
            return Err(ProgramError::UninitializedAccount);
        }

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

        let nonce_bytes = instruction_data.nonce.to_le_bytes();
        ProgramAccount::verify(
            &[
                Seed::from(b"withdraw"),
                Seed::from(accounts.pool_state.key().as_ref()),
                Seed::from(accounts.user.key().as_ref()),
                Seed::from(&nonce_bytes),
            ],
            accounts.user_stake,
            instruction_data.user_stake_bump,
        )?;

        if accounts.user_stake.data_len() != 0 || accounts.user_stake.lamports() != 0 {
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        if accounts.pool_stake.owner() != accounts.stake_program.key() {
            return Err(ProgramError::InvalidAccountData);
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

        let user_lst_data = accounts.user_lst_ata.try_borrow_data()?;
        let user_lst_balance = u64::from_le_bytes(user_lst_data[64..72].try_into().unwrap());

        if user_lst_balance < instruction_data.amount {
            return Err(ProgramError::InsufficientFunds);
        }

        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}

impl<'a> Withdraw<'a> {
    pub const DISCRIMINATOR: u8 = 4;

    pub fn process(&self) -> Result<(), ProgramError> {
        let sol_amount = {
            let pool_state_data = self.accounts.pool_state.try_borrow_data()?;
            let pool_state = PoolState::load(&pool_state_data)?;
            self.calculate_sol_amount(&pool_state)?
        };


        let pool_state_data = self.accounts.pool_state.try_borrow_data()?;
        let pool_state = PoolState::load(&pool_state_data)?;

        let seed_bytes = pool_state.seed.to_le_bytes();
        let pool_bump_binding = [pool_state.bump];
        let pool_seeds = [
            Seed::from(b"lst_pool"),
            Seed::from(pool_state.authority.as_ref()),
            Seed::from(&seed_bytes),
            Seed::from(&pool_bump_binding),
        ];

        let rent = Rent::get()?.minimum_balance(STAKE_ACCOUNT_SIZE as usize);
        let min_stake = rent + MIN_STAKE_DELEGATION;

        if sol_amount < min_stake {
            return Err(ProgramError::InsufficientFunds);
        }

        let pool_after = self
            .accounts
            .pool_stake
            .lamports()
            .checked_sub(sol_amount)
            .ok_or(ProgramError::InsufficientFunds)?;

        if pool_after < min_stake {
            return Err(ProgramError::InsufficientFunds);
        }

        let nonce_bytes = self.instruction_data.nonce.to_le_bytes();
        let user_stake_bump_binding = [self.instruction_data.user_stake_bump];
        let user_stake_seeds = [
            Seed::from(b"withdraw"),
            Seed::from(self.accounts.pool_state.key().as_ref()),
            Seed::from(self.accounts.user.key().as_ref()),
            Seed::from(&nonce_bytes),
            Seed::from(&user_stake_bump_binding),
        ];
        msg!("Creating user stake account");

        create_stake_account(
            self.accounts.user,
            self.accounts.user_stake,
            0,
            &user_stake_seeds,
        )?;

        msg!("Splitting stake account");

        split_stake(
            self.accounts.pool_stake,
            self.accounts.user_stake,
            self.accounts.pool_state,
            &pool_seeds,
            sol_amount,
        )?;

        msg!("Deactivating user stake account");

        deactivate_stake(
            self.accounts.user_stake,
            self.accounts.clock,
            self.accounts.pool_state,
            &pool_seeds,
        )?;

        Burn {
            account: self.accounts.user_lst_ata,
            mint: self.accounts.lst_mint,
            authority: self.accounts.user,
            amount: self.instruction_data.amount,
        }
        .invoke()?;

        drop(pool_state_data);
        let mut pool_state_data = self.accounts.pool_state.try_borrow_mut_data()?;
        let pool_state = PoolState::load_mut(&mut pool_state_data)?;
        pool_state.lst_supply = pool_state
            .lst_supply
            .checked_sub(self.instruction_data.amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        Ok(())
    }

    fn calculate_sol_amount(&self, pool: &PoolState) -> Result<u64, ProgramError> {
        let pool_stake_sol = self
            .accounts
            .pool_stake
            .lamports()
            .saturating_sub(Rent::get()?.minimum_balance(self.accounts.pool_stake.data_len()));

        let reserve_stake_sol = self
            .accounts
            .reserve_stake
            .lamports()
            .saturating_sub(Rent::get()?.minimum_balance(self.accounts.pool_stake.data_len()));


        let total_pool_value = pool_stake_sol
            .checked_add(reserve_stake_sol)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        if pool.lst_supply == 0 {
            return Err(ProgramError::InvalidAccountData);
        }

        let sol_amount = (self.instruction_data.amount as u128)
            .checked_mul(total_pool_value as u128)
            .ok_or(ProgramError::ArithmeticOverflow)?
            .checked_div(pool.lst_supply as u128)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        if sol_amount == 0 {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(sol_amount as u64)
    }
}
