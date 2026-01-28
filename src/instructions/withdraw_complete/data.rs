use pinocchio::program_error::ProgramError;

#[repr(C, packed)]
pub struct WithdrawCompleteInstructionData {
    pub nonce: u64,
}

impl<'a> TryFrom<&'a [u8]> for WithdrawCompleteInstructionData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != size_of::<u64>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let nonce = u64::from_le_bytes(data[0..8].try_into().unwrap());

        if nonce == 0 {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(Self { nonce })
    }
}
