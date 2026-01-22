use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

use crate::{PoolState, ProgramAccount, STAKE_PROGRAM_ID, VOTE_PROGRAM_ID};

pub struct InitializeReserveAccounts<'a> {
    pub pool_state: &'a AccountInfo,
    pub pool_stake: &'a AccountInfo,
    pub reserve_stake: &'a AccountInfo,
    pub validator_vote: &'a AccountInfo,
    pub clock: &'a AccountInfo,
    pub rent: &'a AccountInfo,
    pub stake_history: &'a AccountInfo,
    pub stake_config: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for InitializeReserveAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            pool_state,
            pool_stake,
            reserve_stake,
            validator_vote,
            clock,
            rent,
            stake_history,
            stake_config,
            system_program,
            stake_program,
        ] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        ProgramAccount::check::<PoolState>(pool_state)?;
        ProgramAccount::check_system_program(system_program)?;

        if stake_program.key() != &STAKE_PROGRAM_ID {
            return Err(ProgramError::IncorrectProgramId);
        }

        if validator_vote.owner() != &VOTE_PROGRAM_ID {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self {
            pool_state,
            pool_stake,
            reserve_stake,
            validator_vote,
            clock,
            rent,
            stake_history,
            stake_config,
            system_program,
            stake_program,
        })
    }
}
