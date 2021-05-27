use thiserror::Error;

use solana_program::program_error::ProgramError;

#[derive(Error, Debug, Copy, Clone)]
pub enum StrategyError {
    #[error("Invalid Instruction")]
    InvalidInstruction,
}

impl From<StrategyError> for ProgramError {
    fn from(e: StrategyError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
