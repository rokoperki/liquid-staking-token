use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

use crate::{ProgramAccount, STAKE_PROGRAM_ID};

pub struct MergeReserveAccounts<'a> {
    pub pool_state: &'a AccountInfo,
    pub pool_stake: &'a AccountInfo,
    pub reserve_stake: &'a AccountInfo,
    pub clock: &'a AccountInfo,
    pub stake_history: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for MergeReserveAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            pool_state,
            pool_stake,
            reserve_stake,
            clock,
            stake_history,
            stake_program,
        ] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        ProgramAccount::check(pool_state)?;

        if stake_program.key() != &STAKE_PROGRAM_ID {
            return Err(ProgramError::IncorrectProgramId);
        }

        Ok(Self {
            pool_state,
            pool_stake,
            reserve_stake,
            clock,
            stake_history,
            stake_program,
        })
    }
}
