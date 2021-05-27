use crate::error::StrategyError::InvalidInstruction;
use solana_program::program_error::ProgramError;
use solana_program::{
  instruction::{AccountMeta, Instruction},
  pubkey::Pubkey,
  sysvar,
};

use std::convert::TryInto;
use std::mem::size_of;

// Strategy programs should implement the following interface for strategies.
// TODO(010): Refactor this to share more with VaultInstruction?
pub enum StrategyInstruction {
  /// Deposits a given token into the strategy.
  ///
  /// Accounts expected:
  /// 1. `[]` SPL Token program
  /// 2. `[signer]` The source wallet containing X tokens.
  /// 3. `[]` The target wallet for llX tokens.
  /// 4+ `[]` Source signers
  /// 5+. `[*]` Strategy extra accounts - any additional accounts required by strategy
  /// TODO(009):: Signer pubkeys for multisignature wallets - need signer_num param.
  Deposit { amount: u64 },

  /// Withdraws a token from the strategy.
  ///
  /// Accounts expected:
  /// 1. `[]` SPL Token program
  /// 2. `[signer]` Source Wallet for derivative token (lX).
  /// 3. `[]` Target token (X) wallet target.
  /// 4+ `[]` Source signers
  /// 5+. `[*]` Strategy extra accounts - any additional accounts required by strategy
  /// TODO(009):: Signer pubkeys for multisignature wallets - need signer_num param.
  Withdraw {
    amount: u64, // # of derivative tokens.
  },

  /// Estimates the underlying value of the vault in its native asset.
  ///
  /// This instruction stores its results in a temporary account using the Shared Memory program.
  /// https://spl.solana.com/shared-memory
  ///
  /// Accounts expected:
  /// 1. `[]` Vault program
  /// 1. `[]` Shared memory output
  /// 3+. `[*]` Strategy extra accounts - any additional accounts required by strategy
  EstimateValue {},
}

pub const DEPOSIT: u8 = 0;
pub const WITHDRAW: u8 = 1;
pub const ESTIMATE_VALUE: u8 = 2;

impl StrategyInstruction {
  /// Unpacks a byte buffer into a [VaultInstruction](enum.VaultInstruction.html).
  pub fn unpack(input: &[u8], strategy_instruction: u8) -> Result<Self, ProgramError> {
    let (_tag, rest) = input.split_first().ok_or(InvalidInstruction)?;
    if strategy_instruction == ESTIMATE_VALUE {
      Ok(Self::EstimateValue {})
    } else {
      let amount = rest
        .get(..8)
        .and_then(|slice| slice.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(InvalidInstruction)?;
      if strategy_instruction == DEPOSIT {
        Ok(Self::Deposit { amount })
      } else if  strategy_instruction == WITHDRAW {
        Ok(Self::Withdraw { amount })
      } else {
        return Err(ProgramError::InvalidInstructionData);
      }
    }
  }

  fn pack(&self, instruction_id: u8) -> Vec<u8> {
    let mut buf = Vec::with_capacity(size_of::<Self>());
    buf.push(instruction_id);
    match self {
      &Self::Deposit { amount } => {
        buf.extend_from_slice(&amount.to_le_bytes());
      }
      &Self::Withdraw { amount } => {
        buf.extend_from_slice(&amount.to_le_bytes());
      }
      &Self::EstimateValue {} => {}
    }
    buf
  }

  pub fn deposit(
    instruction_id: u8,
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    source_pubkey: &Pubkey,
    target_pubkey: &Pubkey,
    additional_account_metas: Vec<AccountMeta>,
    amount: u64,
  ) -> Result<Instruction, ProgramError> {
    return create_transfer(
      Self::Deposit { amount }.pack(instruction_id),
      program_id,
      token_program_id,
      source_pubkey,
      target_pubkey,
      additional_account_metas,
    );
  }

  pub fn withdraw(
    instruction_id: u8,
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    source_pubkey: &Pubkey,
    target_pubkey: &Pubkey,
    additional_account_metas: Vec<AccountMeta>,
    amount: u64,
  ) -> Result<Instruction, ProgramError> {
    return create_transfer(
      Self::Withdraw { amount }.pack(instruction_id),
      program_id,
      token_program_id,
      source_pubkey,
      target_pubkey,
      additional_account_metas,
    );
  }

  pub fn estimate_value(
    instruction_id: u8,
    program_id: &Pubkey,
    vault_program_id: &Pubkey,
    shared_memory_account: &Pubkey,
    additional_account_metas: Vec<AccountMeta>,
  ) -> Result<Instruction, ProgramError> {
    create_estimate_value(
      Self::EstimateValue {}.pack(instruction_id),
      program_id,
      vault_program_id,
      shared_memory_account,
      additional_account_metas,
    )
  }
}

pub fn create_estimate_value(
  data: Vec<u8>,
  program_id: &Pubkey,
  vault_program_id: &Pubkey,
  shared_memory_account: &Pubkey,
  additional_account_metas: Vec<AccountMeta>,
) -> Result<Instruction, ProgramError> {
  let mut accounts = vec![
    AccountMeta::new_readonly(*vault_program_id, false),
    AccountMeta::new(*shared_memory_account, false),
  ];
  accounts.extend(additional_account_metas);

  Ok(Instruction {
    program_id: *program_id,
    accounts,
    data,
  })
}

pub fn create_transfer(
  data: Vec<u8>,
  program_id: &Pubkey,
  token_program_id: &Pubkey,
  source_pubkey: &Pubkey,
  target_pubkey: &Pubkey,
  additional_account_metas: Vec<AccountMeta>,
) -> Result<Instruction, ProgramError> {
  let mut accounts = vec![
    AccountMeta::new_readonly(*token_program_id, false),
    AccountMeta::new(*source_pubkey, false),
    AccountMeta::new(*target_pubkey, false),
  ];
  accounts.extend(additional_account_metas);

  Ok(Instruction {
    program_id: *program_id,
    accounts,
    data,
  })
}
