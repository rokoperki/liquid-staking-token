use pinocchio::{account_info::AccountInfo, instruction::Seed, program_error::ProgramError};

use crate::{PoolState, ProgramAccount, WithdrawCompleteAccounts, WithdrawCompleteInstructionData, withdraw_stake};

pub struct WithdrawComplete<'a> {
    pub accounts: WithdrawCompleteAccounts<'a>,
    pub instruction_data: WithdrawCompleteInstructionData,
}

impl<'a> TryFrom<(&[u8], &'a [AccountInfo])> for WithdrawComplete<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&[u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = WithdrawCompleteAccounts::try_from(accounts)?;
        let instruction_data = WithdrawCompleteInstructionData::try_from(data)?;

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

        if accounts.user_stake.owner() != accounts.stake_program.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        if accounts.user_stake.data_len() == 0 || accounts.user_stake.lamports() == 0 {
            return Err(ProgramError::UninitializedAccount);
        }

        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}

impl<'a> WithdrawComplete<'a> {
    pub const DISCRIMINATOR: u8 = 5; // Should be different from Withdraw (which is 4)

    pub fn process(&self) -> Result<(), ProgramError> {
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

        let withdraw_amount = self.accounts.user_stake.lamports();

        withdraw_stake(
            self.accounts.user_stake,
            self.accounts.user,
            self.accounts.pool_state,
            self.accounts.clock,
            self.accounts.stake_history,
            &pool_seeds,
            withdraw_amount,
        )?;

        Ok(())
    }
}
