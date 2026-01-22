use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError};

use crate::{AssociatedToken, Mint, PoolState, ProgramAccount, STAKE_PROGRAM_ID, SignerAccount};

pub struct WithdrawAccounts<'a> {
    pub user: &'a AccountInfo,
    pub pool_state: &'a AccountInfo,
    pub pool_stake: &'a AccountInfo,
    pub reserve_stake: &'a AccountInfo,
    pub user_stake: &'a AccountInfo,
    pub lst_mint: &'a AccountInfo,
    pub user_lst_ata: &'a AccountInfo,
    pub clock: &'a AccountInfo,
    pub rent: &'a AccountInfo,
    pub stake_history: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for WithdrawAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            user,
            pool_state,
            pool_stake,
            reserve_stake,
            user_stake,
            lst_mint,
            user_lst_ata,
            clock,
            rent,
            stake_history,
            system_program,
            stake_program,
            token_program,
        ] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(user)?;
        ProgramAccount::check::<PoolState>(pool_state)?;
        ProgramAccount::check_token_program(token_program)?;
        ProgramAccount::check_system_program(system_program)?;

        Mint::check(lst_mint)?;

        if stake_program.key() != &STAKE_PROGRAM_ID {
            return Err(ProgramError::IncorrectProgramId);
        }

        AssociatedToken::check(
            user_lst_ata,
            *user.key(),
            *lst_mint.key(),
            *token_program.key(),
        )?;

        Ok(Self {
            user,
            pool_state,
            pool_stake,
            reserve_stake,
            user_stake,
            lst_mint,
            user_lst_ata,
            clock,
            rent,
            stake_history,
            system_program,
            stake_program,
            token_program,
        })
    }
}
