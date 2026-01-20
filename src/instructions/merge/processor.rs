use pinocchio::{
    ProgramResult, account_info::AccountInfo, instruction::Seed, program_error::ProgramError,
};

use crate::{MergeAccounts, MergeInstructionData, PoolState, ProgramAccount, merge_stake};

pub struct Merge<'a> {
    pub accounts: MergeAccounts<'a>,
    pub instruction_data: MergeInstructionData,
}

impl<'a> TryFrom<(&[u8], &'a [AccountInfo])> for Merge<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&[u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = MergeAccounts::try_from(accounts)?;
        let instruction_data = MergeInstructionData::try_from(data)?;

        let pool_state_data = accounts.pool_state.try_borrow_data()?;
        let pool_state = PoolState::load(&pool_state_data)?;

        if accounts.pool_stake.key() != &pool_state.stake_account {
            return Err(ProgramError::InvalidAccountData);
        }

        if accounts.deposit_stake.data_is_empty() {
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

        ProgramAccount::verify(
            &[
                Seed::from(b"stake"),
                Seed::from(accounts.pool_state.key().as_ref()),
                Seed::from(accounts.depositor.key().as_ref()),
            ],
            accounts.deposit_stake,
            instruction_data.deposit_stake_bump,
        )?;

        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}

impl<'a> Merge<'a> {
    pub const DISCRIMINATOR: u8 = 2;

    pub fn process(&self) -> ProgramResult {
        let deposit_lamports = {
            let pool_state_data = self.accounts.pool_state.try_borrow_data()?;
            let pool_state = PoolState::load(&pool_state_data)?;

            let seed_binding = pool_state.seed.to_le_bytes();
            let bump_binding = [pool_state.bump];
            let pool_seeds = [
                Seed::from(b"lst_pool"),
                Seed::from(pool_state.authority.as_ref()),
                Seed::from(&seed_binding),
                Seed::from(&bump_binding),
            ];

            let deposit_lampors = self.accounts.deposit_stake.lamports();

            merge_stake(
                self.accounts.pool_stake,
                self.accounts.deposit_stake,
                self.accounts.clock,
                self.accounts.stake_history,
                self.accounts.pool_state,
                &pool_seeds,
            )?;
            deposit_lampors
        };

        {
            let mut pool_state_data = self.accounts.pool_state.try_borrow_mut_data()?;
            let pool_state = PoolState::load_mut(&mut pool_state_data)?;

            pool_state.pending_deposits =
                pool_state.pending_deposits.saturating_sub(deposit_lamports);
        }

        Ok(())
    }
}
