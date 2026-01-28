use pinocchio::program_error::ProgramError;

#[repr(C, packed)]
pub struct WithdrawInstructionData {
    pub amount: u64,
    pub nonce: u64,
}

impl<'a> TryFrom<&'a [u8]> for WithdrawInstructionData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != size_of::<u64>() * 2 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let amount = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let nonce = u64::from_le_bytes(data[8..16].try_into().unwrap());

        if nonce == 0 {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(Self {
            amount,
            nonce,
        })
    }
}
