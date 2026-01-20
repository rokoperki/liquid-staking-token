use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

use crate::{PoolState, ProgramAccount, STAKE_PROGRAM_ID, SignerAccount};

pub struct MergeAccounts<'a> {
    pub payer: &'a AccountInfo,
    pub pool_state: &'a AccountInfo,
    pub pool_stake: &'a AccountInfo,
    pub deposit_stake: &'a AccountInfo,
    pub depositor: &'a AccountInfo,
    pub clock: &'a AccountInfo,
    pub stake_history: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for MergeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            payer,
            pool_state,
            pool_stake,
            deposit_stake,
            depositor,
            clock,
            stake_history,
            stake_program,
        ] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(depositor)?;
        ProgramAccount::check::<PoolState>(pool_state)?;

        if stake_program.key() != &STAKE_PROGRAM_ID {
            return Err(ProgramError::IncorrectProgramId);
        }

        Ok(Self {
            payer,
            pool_state,
            pool_stake,
            deposit_stake,
            depositor,
            clock,
            stake_history,
            stake_program,
        })
    }
}
