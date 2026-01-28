use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

use crate::{AssociatedToken, Mint, ProgramAccount, STAKE_PROGRAM_ID, SignerAccount};

pub struct DepositAccounts<'a> {
    pub depositor: &'a AccountInfo,
    pub pool_state: &'a AccountInfo,
    pub pool_stake: &'a AccountInfo,
    pub reserve_stake: &'a AccountInfo,
    pub lst_mint: &'a AccountInfo,
    pub depositor_lst_ata: &'a AccountInfo,
    /// Programs
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for DepositAccounts<'a> {
    type Error = ProgramError;

    fn try_from(account_infos: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            depositor,
            pool_state,
            pool_stake,
            reserve_stake,
            lst_mint,
            depositor_lst_ata,
            system_program,
            token_program,
            stake_program,
        ] = account_infos
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(depositor)?;
        ProgramAccount::check_system_program(system_program)?;
        ProgramAccount::check_token_program(token_program)?;
        ProgramAccount::check(pool_state)?;
        Mint::check(lst_mint)?;

        AssociatedToken::check(
            depositor_lst_ata,
            *depositor.key(),
            *lst_mint.key(),
            *token_program.key(),
        )?;

        if stake_program.key() != &STAKE_PROGRAM_ID {
            return Err(ProgramError::IncorrectProgramId);
        }

        Ok(Self {
            depositor,
            pool_state,
            pool_stake,
            reserve_stake,
            lst_mint,
            depositor_lst_ata,
            system_program,
            token_program,
            stake_program,
        })
    }
}
