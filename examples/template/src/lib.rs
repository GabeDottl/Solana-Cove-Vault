use solana_program::{
  entrypoint,
  account_info::{next_account_info, AccountInfo},
  entrypoint::ProgramResult,
  instruction::AccountMeta,
  msg,
  program::{invoke, invoke_signed},
  program_error::ProgramError,
  program_option::COption,
  program_pack::{IsInitialized, Pack},
  pubkey::Pubkey,
  sysvar::{rent::Rent, Sysvar},
};

use strategy_api::{error::StrategyError::InvalidInstruction, strategy_instruction::StrategyInstruction};

// TODO:
// * Create Anchor-wrapper
// * Log calls
use solana_program;

entrypoint!(process_instruction);
pub fn process_instruction(
  program_id: &Pubkey,
  accounts: &[AccountInfo],
  instruction_data: &[u8],
) -> ProgramResult {
  msg!("Unpacking instruction");
  let (tag, rest) = instruction_data.split_first().ok_or(InvalidInstruction)?;
  let instruction = StrategyInstruction::unpack(instruction_data, *tag)?;
  let account_info_iter = &mut accounts.iter();
  for (i, account) in account_info_iter.enumerate() {
    msg!("account #{}:  {}", i, account.key);
  }

  match instruction {
    StrategyInstruction::Deposit { amount } => {
      msg!("StrategyInstruction: Deposit {}", amount);
      // TODO(strategist): Implement logic.
      // let account_info_iter = &mut accounts.iter();
      // let token_program = next_account_info(account_info_iter)?;
      // let source_token_account = next_account_info(account_info_iter)?;
      // let target_token_account = next_account_info(account_info_iter)?;

      // DepositToPoolParams
      // https://www.oxygen.org/docs-protocol.html#deposit-assets-to-a-pool
      // https://explorer.solana.com/tx/29d8BexxZBPrTi8vT1y8XHfTYrgaLgmdsStd4XgGGZnwZvqLgnVXVVGvVxWkRJru5hoFS9b83vwCPRBH5uNWpHeW

    }
    StrategyInstruction::Withdraw { amount } => {
      msg!("StrategyInstruction: Withdraw {}", amount);
      // TODO(strategist): Implement logic.
      // let account_info_iter = &mut accounts.iter();
      // let token_program = next_account_info(account_info_iter)?;
      // let source_token_account = next_account_info(account_info_iter)?;
      // let target_token_account = next_account_info(account_info_iter)?;
    }
    StrategyInstruction::EstimateValue {} => {
      msg!("StrategyInstruction: EstimateValue");
      // TODO(strategist): Implement logic.
      // let account_info_iter = &mut accounts.iter();
      // let vault_program = next_account_info(account_info_iter)?;
      // let source_token_account = next_account_info(account_info_iter)?;
    }
  }
  Ok(())
}

// Random based on Token ID's ID. Defines Vault::id().
solana_program::declare_id!("SscrowegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
