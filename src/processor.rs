use solana_program::{
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

use strategy_api::strategy_instruction::StrategyInstruction;
use crate::{
  error::VaultError,
  instruction::{VaultInstruction},
  state::Vault,
};

pub struct Processor;
impl Processor {
  pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
  ) -> ProgramResult {
    msg!("Unpacking instruction");
    let instruction = VaultInstruction::unpack(instruction_data)?;
    // TODO(011): Remove dev logs or gate.
    let account_info_iter = &mut accounts.iter();
    for (i, account) in account_info_iter.enumerate() {
      msg!("account #{}:  {}", i, account.key);
    }

    match instruction {
      VaultInstruction::InitializeVault {
        hodl,
        strategy_program_deposit_instruction_id,
        strategy_program_withdraw_instruction_id,
        strategy_program_estimate_instruction_id,
      } => {
        msg!("Instruction: InitializeVault");
        Self::process_initialize_vault(
          program_id,
          accounts,
          hodl,
          strategy_program_deposit_instruction_id,
          strategy_program_withdraw_instruction_id,
          strategy_program_estimate_instruction_id,
        )
      }
      VaultInstruction::Deposit { amount } => {
        msg!("Instruction: Deposit");
        Self::process_transfer(program_id, accounts, amount, true)
      }
      VaultInstruction::Withdraw { amount } => {
        msg!("Instruction: Withdraw");
        Self::process_transfer(program_id, accounts, amount, false)
      }
      VaultInstruction::EstimateValue {} => {
        msg!("Instruction: EstimateValue");
        Self::process_estimate_value(program_id, accounts)
      }
      VaultInstruction::WriteData {} => {
        msg!("Instruction: WriteData");
        let (_, data) = instruction_data
          .split_first()
          .ok_or(VaultError::InvalidInstruction)?;
        Self::process_write_data(accounts, data)
      }
    }
  }

  fn process_write_data(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // TODO(Security): Ensure we don't screw with the other storage accounts. This should probably
    // be moved to a separate program, like the Shared Memory program...
    let storage_account = next_account_info(account_info_iter)?;
    if storage_account.lamports() > 0 {
      msg!("Data should only be written to temporary accounts");
      // return Err(VaultError::InvalidInstruction.into()); TODO
    }
    if storage_account.data_len() < data.len() {
      msg!("Need more space in storage account");
      return Err(ProgramError::InvalidArgument);
    }
    // Write data into the temporary account storage.
    storage_account.data.borrow_mut().clone_from_slice(data);
    Ok(())
  }

  fn process_initialize_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    hodl: bool,
    strategy_program_deposit_instruction_id: u8,
    strategy_program_withdraw_instruction_id: u8,
    strategy_program_estimate_instruction_id: u8,
  ) -> ProgramResult {
    msg!("Initializing vault");
    let account_info_iter = &mut accounts.iter();

    let initializer = next_account_info(account_info_iter)?;

    if !initializer.is_signer {
      return Err(ProgramError::MissingRequiredSignature);
    }
    let storage_account = next_account_info(account_info_iter)?;
    let lx_token_account = next_account_info(account_info_iter)?;
    let llx_token_mint_id = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let strategy_program = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;

    if *lx_token_account.owner != spl_token::id() || *llx_token_mint_id.owner != spl_token::id() {
      return Err(ProgramError::IncorrectProgramId);
    }

    if !rent.is_exempt(storage_account.lamports(), storage_account.data_len()) {
      return Err(VaultError::NotRentExempt.into());
    }

    let mut storage_info = Vault::unpack_unchecked(&storage_account.data.borrow())?;
    if storage_info.is_initialized() {
      return Err(ProgramError::AccountAlreadyInitialized);
    }

    storage_info.is_initialized = true;
    storage_info.hodl = hodl;
    storage_info.llx_token_mint_id = *llx_token_mint_id.key;
    msg!("Setting auth");
    if hodl {
      msg!("Transferring program X token ownership");
      let x_token_account = next_account_info(account_info_iter)?;
      storage_info.x_token_account = COption::Some(*x_token_account.key);
      // Transfer ownership of the temp account to this program via a derived address.
      let (pda, _bump_seed) = Pubkey::find_program_address(&[b"vault"], program_id);
      let account_owner_change_ix = spl_token::instruction::set_authority(
        token_program.key,
        x_token_account.key,
        Some(&pda),
        spl_token::instruction::AuthorityType::AccountOwner,
        initializer.key,
        &[&initializer.key],
      )?;
      invoke(
        &account_owner_change_ix,
        &[
          x_token_account.clone(),
          initializer.clone(),
          token_program.clone(),
        ],
      )?;
    }
    println!("strategy_program.key: {}", strategy_program.key);
    storage_info.strategy_program_id = *strategy_program.key;
    storage_info.strategy_program_deposit_instruction_id = strategy_program_deposit_instruction_id;
    storage_info.strategy_program_withdraw_instruction_id =
      strategy_program_withdraw_instruction_id;
    storage_info.strategy_program_estimate_instruction_id =
      strategy_program_estimate_instruction_id;
    storage_info.last_estimated_value = 0;

    // Write the info to the actual account.
    Vault::pack(storage_info, &mut storage_account.data.borrow_mut())?;

    // Transfer ownership of the temp account to this program via a derived address.
    let (pda, _bump_seed) = Pubkey::find_program_address(&[b"vault"], program_id);
    let account_owner_change_ix = spl_token::instruction::set_authority(
      token_program.key,
      lx_token_account.key,
      Some(&pda),
      spl_token::instruction::AuthorityType::AccountOwner,
      initializer.key,
      &[&initializer.key],
    )?;

    msg!("Calling the token program to transfer X vault token account ownership");
    msg!(
      "Token program: {}. Transferring ownership {} -> {}",
      token_program.key,
      initializer.key,
      pda
    );
    invoke(
      &account_owner_change_ix,
      &[
        lx_token_account.clone(),
        initializer.clone(),
        token_program.clone(),
      ],
    )?;
    let mint_owner_change_ix = spl_token::instruction::set_authority(
      token_program.key,
      llx_token_mint_id.key,
      Some(&pda),
      spl_token::instruction::AuthorityType::MintTokens,
      initializer.key,
      &[&initializer.key],
    )?;

    msg!("Calling the token program to transfer llX token mint authority");
    msg!(
      "Token program: {}. Transferring minting control {} -> {}",
      token_program.key,
      initializer.key,
      pda
    );
    invoke(
      &mint_owner_change_ix,
      &[
        llx_token_mint_id.clone(),
        initializer.clone(),
        token_program.clone(),
      ],
    )?;
    Ok(())
  }

  fn process_transfer(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
    is_deposit: bool,
  ) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let token_program = next_account_info(account_info_iter)?;
    let source_token_account = next_account_info(account_info_iter)?;
    let target_token_account = next_account_info(account_info_iter)?;
    // Additional account metas:
    // TODO(009): Support more than one source authority.
    let source_authority = next_account_info(account_info_iter)?;
    let storage_account = next_account_info(account_info_iter)?;
    let strategy_program = next_account_info(account_info_iter)?;

    let storage_info = Vault::unpack_unchecked(&storage_account.data.borrow())?;
    if !storage_info.is_initialized() {
      msg!("Storage not configured!");
      return Err(VaultError::InvalidInstruction.into());
    }

    if *strategy_program.key != storage_info.strategy_program_id {
      msg!("Invalid strategy program provided!");
      return Err(VaultError::InvalidInstruction.into());
    }

    // Charge fees
    if is_deposit {
      // TODO(001): implement.
      msg!("Mint llX tokens to client account");
    } else {
      // TODO(002): implement.
      msg!("Transfer & burn llX tokens from client");
    }

    let (pda, bump_seed) = Pubkey::find_program_address(&[b"vault"], program_id);
    // Check if this is a HODL Vault; if so, we deposit & withdraw from
    if storage_info.hodl {
      let x_token_account = next_account_info(account_info_iter)?;
      msg!("Calling the token program to transfer tokens");
      if is_deposit {
        let transfer_to_vault_ix = spl_token::instruction::transfer(
          token_program.key,
          source_token_account.key,
          x_token_account.key,
          &source_authority.key,
          &[&source_authority.key],
          amount,
        )?;
        msg!("Depositing to hodl account");
        invoke(
          &transfer_to_vault_ix,
          &[
            source_token_account.clone(),
            x_token_account.clone(),
            source_authority.clone(),
            token_program.clone(),
          ],
        )?;
      } else {
        let transfer_to_client_ix = spl_token::instruction::transfer(
          token_program.key,
          x_token_account.key,
          target_token_account.key,
          &pda,
          &[&pda],
          amount,
        )?;
        msg!("Withdrawing from hodl account");
        invoke_signed(
          &transfer_to_client_ix,
          &[
            x_token_account.clone(),
            target_token_account.clone(),
            source_authority.clone(),
            token_program.clone(),
          ],
          &[&[&b"vault"[..], &[bump_seed]]],
        )?;
      }
    } else {
      // Pass through the source authority above the extra signers.
      let mut account_metas = vec![AccountMeta::new_readonly(*source_authority.key, true)];
      account_metas.extend(
        account_info_iter
          .map(|account| {
            if account.is_writable {
              AccountMeta::new(*account.key, account.is_signer)
            } else {
              AccountMeta::new_readonly(*account.key, account.is_signer)
            }
          })
          .collect::<Vec<AccountMeta>>(),
      );

      if is_deposit {
        msg!(
          "Depositing into strategy {}",
          storage_info.strategy_program_deposit_instruction_id
        );
        let instruction = StrategyInstruction::deposit(
          storage_info.strategy_program_deposit_instruction_id,
          program_id,
          &token_program.key,
          &source_token_account.key,
          &target_token_account.key,
          // Pass along any additional accounts.
          account_metas,
          amount,
        )?;
        invoke(&instruction, &accounts)?;
      } else {
        msg!(
          "Withdrawing from strategy {}",
          storage_info.strategy_program_withdraw_instruction_id
        );
        let instruction = StrategyInstruction::withdraw(
          storage_info.strategy_program_withdraw_instruction_id,
          program_id,
          &token_program.key,
          &source_token_account.key,
          &target_token_account.key,
          // Pass along any additional accounts.
          account_metas,
          amount,
        )?;
        invoke_signed(&instruction, &accounts, &[&[&b"vault"[..], &[bump_seed]]])?;
      }
    }
    Ok(())
  }

  fn process_estimate_value(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    msg!(
      "Estimate Value!--------------------------------------------------------------------------"
    );
    let account_info_iter = &mut accounts.iter();
    let _ = next_account_info(account_info_iter)?; // program
    let temp_memory_account = next_account_info(account_info_iter)?;
    let storage_account = next_account_info(account_info_iter)?;

    msg!("Unpacking storage {}", storage_account.key);
    let storage_info = Vault::unpack_unchecked(&storage_account.data.borrow())?;
    msg!("Unpacked storage");
    if !storage_info.is_initialized() {
      msg!("Storage not configured!");
      return Err(VaultError::InvalidInstruction.into());
    }

    if storage_info.hodl {
      // Derive the value directly from the storage account.
      let x_token_account = next_account_info(account_info_iter)?;
      let internal_account =
        spl_token::state::Account::unpack_unchecked(&x_token_account.data.borrow()).unwrap();
      msg!(
        "Estimating value from HODL vault: {}",
        internal_account.amount
      );
      let instruction = VaultInstruction::write_data(
        program_id,
        temp_memory_account.key,
        &internal_account.amount.to_le_bytes(),
      )?;
      invoke(&instruction, &accounts)?;
    } else {
      // Estimating value from a strategy.
      let strategy_program = next_account_info(account_info_iter)?;
      if *strategy_program.key != storage_info.strategy_program_id {
        msg!(
          "Invalid strategy program provided! Got: {} expected {}",
          strategy_program.key,
          storage_info.strategy_program_id
        );
        return Err(VaultError::InvalidInstruction.into());
      }
      let account_metas = account_info_iter
        .map(|account| {
          if account.is_writable {
            AccountMeta::new(*account.key, account.is_signer)
          } else {
            AccountMeta::new_readonly(*account.key, account.is_signer)
          }
        })
        .collect::<Vec<AccountMeta>>();
      msg!(
        "Estimating value on strategy program! {}",
        storage_info.strategy_program_estimate_instruction_id
      );
      let instruction = StrategyInstruction::estimate_value(
        storage_info.strategy_program_estimate_instruction_id,
        strategy_program.key,
        program_id,
        temp_memory_account.key,
        account_metas,
      )?;
      invoke(&instruction, &accounts)?;
    }
    Ok(())
  }
}
