use pinocchio::{
    ProgramResult, account_info::AccountInfo, instruction::Seed, program_error::ProgramError,
};

use crate::{MergeReserveAccounts, PoolState, ProgramAccount, merge_stake};

pub struct MergeReserve<'a> {
    pub accounts: MergeReserveAccounts<'a>,
}

impl<'a> TryFrom<&'a [AccountInfo]> for MergeReserve<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let accounts = MergeReserveAccounts::try_from(accounts)?;

        let pool_state_data = accounts.pool_state.try_borrow_data()?;
        let pool_state = PoolState::load(&pool_state_data)?;

        if pool_state.discriminator == 0 {
            return Err(ProgramError::UninitializedAccount);
        }

        let seed_bytes = pool_state.seed.to_le_bytes();
        ProgramAccount::verify(
            &[
                Seed::from(b"lst_pool"),
                Seed::from(&seed_bytes),
            ],
            accounts.pool_state,
            pool_state.bump,
        )?;
        if accounts.pool_stake.owner() != accounts.stake_program.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        if accounts.reserve_stake.owner() != accounts.stake_program.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        if accounts.pool_stake.key() != &pool_state.stake_account {
            return Err(ProgramError::InvalidAccountData);
        }

        if accounts.reserve_stake.key() != &pool_state.reserve_stake {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self { accounts })
    }
}

impl<'a> MergeReserve<'a> {
    pub const DISCRIMINATOR: u8 = 3;

    pub fn process(&self) -> ProgramResult {
        let pool_state_data = self.accounts.pool_state.try_borrow_data()?;
        let pool_state = PoolState::load(&pool_state_data)?;

        let seed_binding = pool_state.seed.to_le_bytes();
        let binding = [pool_state.bump];
        let pool_seeds = [
            Seed::from(b"lst_pool"),
            Seed::from(&seed_binding),
            Seed::from(&binding),
        ];

        if self.accounts.reserve_stake.lamports() == 0 {
            return Err(ProgramError::UninitializedAccount);
        }

        merge_stake(
            self.accounts.pool_stake,
            self.accounts.reserve_stake,
            self.accounts.clock,
            self.accounts.stake_history,
            self.accounts.pool_state,
            &pool_seeds,
        )?;

        Ok(())
    }
}
