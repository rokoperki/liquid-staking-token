use crate::PinocchioError;
use pinocchio::{program_error::ProgramError};

#[repr(C, packed)]
pub struct InitializeData {
    pub seed: u64,
    pub pool_bump: u8,
    pub mint_bump: u8,
    pub stake_bump: u8,
    pub reserve_bump: u8,
}

impl<'a> TryFrom<&'a [u8]> for InitializeData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != size_of::<u64>() + 4 * size_of::<u8>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let (seed, pool_bump, mint_bump, stake_bump, reserve_bump) = (
            u64::from_le_bytes(data[0..8].try_into().unwrap()),
            u8::from_le_bytes(data[8..9].try_into().unwrap()),
            u8::from_le_bytes(data[9..10].try_into().unwrap()),
            u8::from_le_bytes(data[10..11].try_into().unwrap()),
            u8::from_le_bytes(data[11..12].try_into().unwrap()),
        );

        if seed == 0 {
            return Err(PinocchioError::InvalidSeed.into());
        }

        Ok(Self {
            seed,
            pool_bump,
            mint_bump,
            stake_bump,
            reserve_bump,
        })
    }
}
