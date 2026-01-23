use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

use crate::{PoolState, ProgramAccount, STAKE_PROGRAM_ID, SignerAccount};

pub struct WithdrawCompleteAccounts<'a> {
    pub user: &'a AccountInfo,
    pub pool_state: &'a AccountInfo,
    pub user_stake: &'a AccountInfo,
    pub clock: &'a AccountInfo,
    pub stake_history: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for WithdrawCompleteAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            user,
            pool_state,
            user_stake,
            clock,
            stake_history,
            stake_program,
        ] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(user)?;
        ProgramAccount::check::<PoolState>(pool_state)?;

        if stake_program.key() != &STAKE_PROGRAM_ID {
            return Err(ProgramError::IncorrectProgramId);
        }

        Ok(Self {
            user,
            pool_state,
            user_stake,
            clock,
            stake_history,
            stake_program,
        })
    }
}
