use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

use crate::{
    AssociatedToken, Mint, PoolState, ProgramAccount, STAKE_PROGRAM_ID, SignerAccount,
    VOTE_PROGRAM_ID,
};

pub struct DepositAccounts<'a> {
    pub depositor: &'a AccountInfo,
    pub pool_state: &'a AccountInfo,
    pub deposit_stake: &'a AccountInfo,
    pub pool_stake: &'a AccountInfo,
    pub validator_vote: &'a AccountInfo,
    pub lst_mint: &'a AccountInfo,
    pub depositor_lst_ata: &'a AccountInfo,
    /// Sysvar accounts
    pub clock: &'a AccountInfo,
    pub rent: &'a AccountInfo,
    pub stake_history: &'a AccountInfo,
    pub stake_config: &'a AccountInfo,
    /// Programs
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
    pub stake_program: &'a AccountInfo,
    pub ata_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for DepositAccounts<'a> {
    type Error = ProgramError;

    fn try_from(account_infos: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            depositor,
            pool_state,
            deposit_stake,
            pool_stake,
            validator_vote,
            lst_mint,
            depositor_lst_ata,
            clock,
            rent,
            stake_history,
            stake_config,
            system_program,
            token_program,
            stake_program,
            ata_program,
        ] = account_infos
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(depositor)?;
        ProgramAccount::check_system_program(system_program)?;
        ProgramAccount::check_token_program(token_program)?;
        ProgramAccount::check_ata_program(ata_program)?;
        ProgramAccount::check::<PoolState>(pool_state)?;
        Mint::check(lst_mint)?;
        AssociatedToken::init_if_needed(
            depositor_lst_ata,
            lst_mint,
            depositor,
            depositor,
            system_program,
            token_program,
        )?;

        if stake_program.key() != &STAKE_PROGRAM_ID {
            return Err(ProgramError::IncorrectProgramId);
        }

        if validator_vote.owner() != &VOTE_PROGRAM_ID {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self {
            depositor,
            pool_state,
            deposit_stake,
            pool_stake,
            validator_vote,
            lst_mint,
            depositor_lst_ata,
            clock,
            rent,
            stake_history,
            stake_config,
            system_program,
            token_program,
            stake_program,
            ata_program,
        })
    }
}
