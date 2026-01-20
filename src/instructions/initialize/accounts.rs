use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError};

use crate::{
    ProgramAccount, SignerAccount,
    constants::{STAKE_PROGRAM_ID, VOTE_PROGRAM_ID},
};

pub struct InitializeAccounts<'a> {
    pub initializer: &'a AccountInfo,
    pub pool_state: &'a AccountInfo,
    pub lst_mint: &'a AccountInfo,
    pub stake_account: &'a AccountInfo,
    pub validator_vote: &'a AccountInfo,
    pub clock: &'a AccountInfo,
    pub rent: &'a AccountInfo,
    pub stake_history: &'a AccountInfo,
    pub stake_config: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for InitializeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            initializer,
            pool_state,
            lst_mint,
            stake_account,
            validator_vote,
            clock,
            rent,
            stake_history,
            stake_config,
            system_program,
            token_program,
            stake_program,
        ] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(initializer)?;
        ProgramAccount::check_system_program(system_program)?;
        ProgramAccount::check_token_program(token_program)?;

        if stake_program.key() != &STAKE_PROGRAM_ID {
            return Err(ProgramError::IncorrectProgramId);
        }

        if validator_vote.owner() != &VOTE_PROGRAM_ID {
            msg!("Invalid validator vote account");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self {
            initializer,
            pool_state,
            lst_mint,
            stake_account,
            validator_vote,
            clock,
            rent,
            stake_history,
            stake_config,
            system_program,
            token_program,
            stake_program,
        })
    }
}
