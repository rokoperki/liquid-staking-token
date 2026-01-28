use pinocchio::{
    ProgramResult, account_info::AccountInfo, instruction::Seed, program_error::ProgramError,
};

use crate::{
    InitializeReserveAccounts, MIN_STAKE_DELEGATION, PoolState, ProgramAccount, STAKE_ACCOUNT_SIZE, delegate_stake, initialize_stake, reinit_stake_account
};

pub struct InitializeReserve<'a> {
    pub accounts: InitializeReserveAccounts<'a>,
}

impl<'a> TryFrom<&'a [AccountInfo]> for InitializeReserve<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let accounts = InitializeReserveAccounts::try_from(accounts)?;

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

        if accounts.pool_stake.key() != &pool_state.stake_account {
            return Err(ProgramError::InvalidAccountData);
        }

        if accounts.reserve_stake.key() != &pool_state.reserve_stake {
            return Err(ProgramError::InvalidAccountData);
        }

        if accounts.validator_vote.key() != &pool_state.validator_vote {
            return Err(ProgramError::InvalidAccountData);
        }

        if accounts.reserve_stake.lamports() < (STAKE_ACCOUNT_SIZE + MIN_STAKE_DELEGATION) {
            return Err(ProgramError::InsufficientFunds);
        }

        Ok(Self { accounts })
    }
}

impl<'a> InitializeReserve<'a> {
    pub const DISCRIMINATOR: u8 = 2;

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

        if self.accounts.reserve_stake.data_len() > 0 {
            let reserve_data = self.accounts.reserve_stake.try_borrow_data()?;
            if reserve_data.len() >= 4 {
                let state = u32::from_le_bytes(reserve_data[0..4].try_into().unwrap());
                if state >= 1 {
                    return Err(ProgramError::AccountAlreadyInitialized);
                }
            }
        }
    
        // Reallocate if needed (account was created with 0 space)
        if self.accounts.reserve_stake.data_len() == 0 {
            let reserve_bump_binding = [pool_state.reserve_bump];
            let reserve_seeds = [
                Seed::from(b"reserve_stake"),
                Seed::from(self.accounts.pool_state.key().as_ref()),
                Seed::from(&reserve_bump_binding),
            ];
    
            reinit_stake_account(self.accounts.reserve_stake, &reserve_seeds)?;
        }

        initialize_stake(
            self.accounts.reserve_stake,
            self.accounts.rent,
            self.accounts.pool_state,
            self.accounts.pool_state,
        )?;

        delegate_stake(
            self.accounts.reserve_stake,
            self.accounts.validator_vote,
            self.accounts.clock,
            self.accounts.stake_history,
            self.accounts.stake_config,
            self.accounts.pool_state,
            &pool_seeds,
        )?;

        Ok(())
    }
}
