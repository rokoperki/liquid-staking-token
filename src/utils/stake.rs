// src/helpers/stake.rs (or src/utils/stake.rs)

use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    msg,
    sysvars::{Sysvar, rent::Rent},
};
use pinocchio_system::instructions::{Allocate, Assign, CreateAccount};

use crate::constants::*;

/// Create a stake account with PDA signing
pub fn create_stake_account<'a>(
    payer: &'a AccountInfo,
    stake_account: &'a AccountInfo,
    lamports: u64,
    seeds: &[Seed],
) -> ProgramResult {
    let signer = [Signer::from(seeds)];
    let rent = Rent::get()?;

    CreateAccount {
        from: payer,
        to: stake_account,
        lamports: rent.minimum_balance(STAKE_ACCOUNT_SIZE as usize) + lamports,
        space: STAKE_ACCOUNT_SIZE,
        owner: &STAKE_PROGRAM_ID,
    }
    .invoke_signed(&signer)?;

    msg!("Stake account created");
    Ok(())
}

pub fn reinit_stake_account<'a>(stake_account: &'a AccountInfo, seeds: &[Seed]) -> ProgramResult {
    let signer = [Signer::from(seeds)];

    // Allocate 200 bytes
    Allocate {
        account: stake_account,
        space: STAKE_ACCOUNT_SIZE,
    }
    .invoke_signed(&signer)?;

    // Assign to stake program
    Assign {
        account: stake_account,
        owner: &STAKE_PROGRAM_ID,
    }
    .invoke_signed(&signer)?;

    msg!("Stake account re-prepared");
    Ok(())
}

/// Initialize stake account with staker and withdrawer authorities (no lockup)
pub fn initialize_stake<'a>(
    stake_account: &'a AccountInfo,
    rent: &'a AccountInfo,
    staker: &'a AccountInfo,
    withdrawer: &'a AccountInfo,
) -> ProgramResult {
    let mut data = [0u8; 116];

    // Discriminator (0 = Initialize)
    data[0..4].copy_from_slice(&0u32.to_le_bytes());

    // Authorized
    data[4..36].copy_from_slice(staker.key().as_ref());
    data[36..68].copy_from_slice(withdrawer.key().as_ref());

    // Lockup (zeros = no lockup)
    data[68..76].copy_from_slice(&0i64.to_le_bytes()); // unix_timestamp
    data[76..84].copy_from_slice(&0u64.to_le_bytes()); // epoch
    data[84..116].copy_from_slice(&[0u8; 32]); // custodian

    let ix = Instruction {
        program_id: &STAKE_PROGRAM_ID,
        accounts: &[
            AccountMeta {
                pubkey: stake_account.key(),
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: rent.key(),
                is_signer: false,
                is_writable: false,
            },
        ],
        data: &data,
    };

    pinocchio::program::invoke(&ix, &[stake_account, rent])?;
    msg!("Stake account initialized");
    Ok(())
}

/// Delegate stake to a validator
pub fn delegate_stake<'a>(
    stake_account: &'a AccountInfo,
    validator_vote: &'a AccountInfo,
    clock: &'a AccountInfo,
    stake_history: &'a AccountInfo,
    stake_config: &'a AccountInfo,
    staker: &'a AccountInfo,
    signer_seeds: &[Seed],
) -> ProgramResult {
    let signer = [Signer::from(signer_seeds)];
    let data = 2u32.to_le_bytes();

    let ix = Instruction {
        program_id: &STAKE_PROGRAM_ID,
        accounts: &[
            AccountMeta {
                pubkey: stake_account.key(),
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: validator_vote.key(),
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: clock.key(),
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: stake_history.key(),
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: stake_config.key(),
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: staker.key(),
                is_signer: true,
                is_writable: false,
            },
        ],
        data: &data,
    };

    pinocchio::program::invoke_signed(
        &ix,
        &[
            stake_account,
            validator_vote,
            clock,
            stake_history,
            stake_config,
            staker,
        ],
        &signer,
    )?;

    msg!("Stake delegated");
    Ok(())
}

/// Merge source stake into destination stake
pub fn merge_stake<'a>(
    destination: &'a AccountInfo,
    source: &'a AccountInfo,
    clock: &'a AccountInfo,
    stake_history: &'a AccountInfo,
    staker: &'a AccountInfo,
    signer_seeds: &[Seed],
) -> ProgramResult {
    let signer = [Signer::from(signer_seeds)];
    let data = 7u32.to_le_bytes();

    let ix = Instruction {
        program_id: &STAKE_PROGRAM_ID,
        accounts: &[
            AccountMeta {
                pubkey: destination.key(),
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: source.key(),
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: clock.key(),
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: stake_history.key(),
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: staker.key(),
                is_signer: true,
                is_writable: false,
            },
        ],
        data: &data,
    };

    pinocchio::program::invoke_signed(
        &ix,
        &[destination, source, clock, stake_history, staker],
        &signer,
    )?;

    msg!("Stakes merged");
    Ok(())
}

pub fn split_stake<'a>(
    destination: &'a AccountInfo,
    source: &'a AccountInfo,
    staker: &'a AccountInfo,
    signer_seeds: &[Seed],
    amount: u64,
) -> ProgramResult {
    let signer = [Signer::from(signer_seeds)];
    let mut data = [0u8; 12];
    data[0..4].copy_from_slice(&3u32.to_le_bytes());
    data[4..12].copy_from_slice(&amount.to_le_bytes());

    let ix = Instruction {
        program_id: &STAKE_PROGRAM_ID,
        accounts: &[
            AccountMeta {
                pubkey: destination.key(),
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: source.key(),
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: staker.key(),
                is_signer: true,
                is_writable: false,
            },
        ],
        data: &data,
    };

    pinocchio::program::invoke_signed(&ix, &[destination, source, staker], &signer)?;

    msg!("Stakes merged");
    Ok(())
}

pub fn deactivate_stake<'a>(
    stake_account: &'a AccountInfo,
    clock: &'a AccountInfo,
    staker: &'a AccountInfo,
    signer_seeds: &[Seed],
) -> ProgramResult {
    let signer = [Signer::from(signer_seeds)];
    let data = 5u32.to_le_bytes(); // Deactivate discriminator, no extra data

    let ix = Instruction {
        program_id: &STAKE_PROGRAM_ID,
        accounts: &[
            AccountMeta {
                pubkey: stake_account.key(),
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: clock.key(),
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: staker.key(),
                is_signer: true,
                is_writable: false,
            },
        ],
        data: &data,
    };

    pinocchio::program::invoke_signed(&ix, &[stake_account, clock, staker], &signer)?;

    msg!("Stake deactivated");
    Ok(())
}

pub fn withdraw_stake<'a>(
    source: &'a AccountInfo,
    destination: &'a AccountInfo,
    staker: &'a AccountInfo,
    clock: &'a AccountInfo,
    stake_history: &'a AccountInfo,
    signer_seeds: &[Seed],
    amount: u64,
) -> ProgramResult {
    let signer = [Signer::from(signer_seeds)];
    let mut data = [0u8; 12];
    data[0..4].copy_from_slice(&4u32.to_le_bytes());
    data[4..12].copy_from_slice(&amount.to_le_bytes());

    let ix = Instruction {
        program_id: &STAKE_PROGRAM_ID,
        accounts: &[
            AccountMeta {
                pubkey: source.key(),
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: destination.key(),
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: clock.key(),
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: stake_history.key(),
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: staker.key(),
                is_signer: true,
                is_writable: false,
            },
        ],
        data: &data,
    };

    pinocchio::program::invoke_signed(
        &ix,
        &[source, destination, clock, stake_history, staker],
        &signer,
    )?;

    msg!("Stake withdrawn");
    Ok(())
}
