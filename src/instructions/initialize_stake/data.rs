use crate::PinocchioError;
use pinocchio::{program_error::ProgramError};

#[repr(C, packed)]
pub struct InitializeData {
    pub seed: u64,
}

impl<'a> TryFrom<&'a [u8]> for InitializeData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != size_of::<u64>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let seed = u64::from_le_bytes(data[0..8].try_into().unwrap());

        if seed == 0 {
            return Err(PinocchioError::InvalidSeed.into());
        }

        Ok(Self {
            seed,
        })
    }
}
