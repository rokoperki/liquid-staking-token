use pinocchio::program_error::ProgramError;

pub struct MergeInstructionData {
    pub deposit_stake_bump: u8,
}

impl<'a> TryFrom<&'a [u8]> for MergeInstructionData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != size_of::<u8>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let deposit_stake_bump = u8::from_le_bytes(data[0..1].try_into().unwrap());

        Ok(Self {
            deposit_stake_bump,
        })
    }
}
