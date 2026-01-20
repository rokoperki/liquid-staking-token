use crate::PinocchioError;
use pinocchio::program_error::ProgramError;

#[repr(C, packed)]
pub struct InitializeData {
    pub seed: u64,
    pub pool_bump: u8,
    pub mint_bump: u8,
    pub stake_bump: u8,
}

impl InitializeData {
    pub const SIZE: usize = 11;
}

impl<'a> TryFrom<&'a [u8]> for InitializeData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != Self::SIZE {
            return Err(ProgramError::InvalidInstructionData);
        }

        let seed = u64::from_le_bytes(data[0..8].try_into().unwrap());
        if seed == 0 {
            return Err(PinocchioError::InvalidSeed.into());
        }

        Ok(Self {
            seed,
            pool_bump: data[8],
            mint_bump: data[9],
            stake_bump: data[10],
        })
    }
}
