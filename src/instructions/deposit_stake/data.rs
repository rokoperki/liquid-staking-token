use pinocchio::program_error::ProgramError;

use crate::MIN_STAKE_DELEGATION;

#[repr(C, packed)]
pub struct DepositInstructionData {
    pub amount: u64,
    pub deposit_stake_bump: u8,
}

impl<'a> TryFrom<&'a [u8]> for DepositInstructionData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != size_of::<u64>() + size_of::<u8>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let amount = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let deposit_stake_bump = u8::from_le_bytes(data[8..9].try_into().unwrap());

        if amount == 0 {
            return Err(ProgramError::InvalidInstructionData);
        }

        if amount < MIN_STAKE_DELEGATION {
            return Err(ProgramError::InvalidArgument);
        }

        Ok(Self {
            amount,
            deposit_stake_bump,
        })
    }
}
