use pinocchio::{
    ProgramResult, account_info::AccountInfo, entrypoint, program_error::ProgramError,
    pubkey::Pubkey,
};
entrypoint!(process_instruction);

pub mod instructions;
pub use instructions::*;

pub mod error;
pub use error::*;

pub mod states;
pub use states::*;

pub mod constants;
pub use constants::*;

pub mod utils;
pub use utils::*;

pub const ID: Pubkey = [
    0x0f, 0x1e, 0x6b, 0x14, 0x21, 0xc0, 0x4a, 0x07, 0x04, 0x31, 0x26, 0x5c, 0x19, 0xc5, 0xbb, 0xee,
    0x19, 0x92, 0xba, 0xe8, 0xaf, 0xd1, 0xcd, 0x07, 0x8e, 0xf8, 0xaf, 0x70, 0x47, 0xdc, 0x11, 0xf7,
];

fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data.split_first() {
        Some((&Initialize::DISCRIMINATOR, data)) => {
            Initialize::try_from((data, accounts))?.process()
        }
        Some((&Deposit::DISCRIMINATOR, data)) => Deposit::try_from((data, accounts))?.process(),
        Some((&InitializeReserve::DISCRIMINATOR, _data)) => {
            InitializeReserve::try_from(accounts)?.process()
        }
        Some((&MergeReserve::DISCRIMINATOR, _data)) => MergeReserve::try_from(accounts)?.process(),
        Some((&Withdraw::DISCRIMINATOR, data)) => Withdraw::try_from((data, accounts))?.process(),

        _ => Err(ProgramError::InvalidInstructionData),
    }
}
