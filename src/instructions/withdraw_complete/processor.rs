use pinocchio::{account_info::AccountInfo, instruction::Seed, program_error::ProgramError, pubkey::find_program_address};

use crate::{PoolState, ProgramAccount, WithdrawCompleteAccounts, WithdrawCompleteInstructionData, withdraw_stake};

pub struct WithdrawComplete<'a> {
    pub accounts: WithdrawCompleteAccounts<'a>,
    pub instruction_data: WithdrawCompleteInstructionData,
    pub user_stake_bump: u8,
}

impl<'a> TryFrom<(&[u8], &'a [AccountInfo])> for WithdrawComplete<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&[u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = WithdrawCompleteAccounts::try_from(accounts)?;
        let instruction_data = WithdrawCompleteInstructionData::try_from(data)?;

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

        let nonce_bytes = instruction_data.nonce.to_le_bytes();
        let (user_stake_pda, user_stake_bump) = find_program_address(
            &[
                b"withdraw",
                accounts.pool_state.key().as_ref(),
                accounts.user.key().as_ref(),
                &nonce_bytes,
            ],
            &crate::ID,
        );
        if accounts.user_stake.key() != &user_stake_pda {
            return Err(ProgramError::InvalidSeeds);
        }

        if accounts.user_stake.owner() != accounts.stake_program.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        if accounts.user_stake.data_len() == 0 || accounts.user_stake.lamports() == 0 {
            return Err(ProgramError::UninitializedAccount);
        }

        Ok(Self {
            accounts,
            instruction_data,
            user_stake_bump,
        })
    }
}

impl<'a> WithdrawComplete<'a> {
    pub const DISCRIMINATOR: u8 = 5; 

    pub fn process(&self) -> Result<(), ProgramError> {
        let pool_state_data = self.accounts.pool_state.try_borrow_data()?;
        let pool_state = PoolState::load(&pool_state_data)?;

        let seed_bytes = pool_state.seed.to_le_bytes();
        let pool_bump_binding = [pool_state.bump];
        let pool_seeds = [
            Seed::from(b"lst_pool"),
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
