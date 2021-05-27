#![cfg(feature = "test-bpf")]

use {
  template,
  assert_matches::*,
  solana_program::{
    instruction::{AccountMeta},
    program_option::COption,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
  },
  solana_program_test::{processor, ProgramTest, ProgramTestContext},
  solana_sdk::signature::Keypair,
  solana_sdk::{account::Account, signature::Signer, transaction::Transaction},
  spl_token::{processor::Processor},
};
use strategy_api::{error::StrategyError::InvalidInstruction, strategy_instruction::{DEPOSIT, WITHDRAW, ESTIMATE_VALUE, StrategyInstruction}};


use std::convert::TryInto;

#[tokio::test]
async fn test() {
  // Start the test client
  let mut program_test = ProgramTest::new(
    "token_test",
    spl_token::id(),
    processor!(Processor::process),
  );
  program_test.add_program(
    "template_test",
    ::template::id(),
    processor!(::template::process_instruction),
  );
  
  let mut program_test_context = program_test.start_with_context().await;
  // A basic Vault has 3 relevant tokens: X (underlying asset), lX (strategy derivative), llX (vault
  // derivative). We roughly need a client-managed & vault-managed SPL token account per-token.
  // For succintnesss, we set all of these up together:
  let mint_client_vault_accounts =
    create_tokens_and_accounts(&mut program_test_context, 1, 3).await;


  let mut transaction = Transaction::new_with_payer(
    &[
      StrategyInstruction::deposit(
        DEPOSIT,
        &::template::id(),
        &spl_token::id(),
        &mint_client_vault_accounts[0][1].pubkey(), // Client X token account
        &mint_client_vault_accounts[0][2].pubkey(), // Strategy X token account
        vec![],
        99 // amount
      )
      .unwrap(),
      StrategyInstruction::withdraw(
        WITHDRAW,
        &::template::id(),
        &spl_token::id(),
        &mint_client_vault_accounts[0][1].pubkey(), // Client X token account
        &mint_client_vault_accounts[0][2].pubkey(), // Strategy X token account
        vec![],
        99 // Amount of lX tokens being used 
      )
      .unwrap(),
      StrategyInstruction::estimate_value(
        ESTIMATE_VALUE,
        &::template::id(),
        &spl_token::id(),  // TODO: Vault/memory program
        &spl_token::id(),  // TODO: Memory storage
        vec![],
      )
      .unwrap(),
    ],
    Some(&program_test_context.payer.pubkey()),
  );
  transaction.sign(
    &[&program_test_context.payer],
    program_test_context.last_blockhash,
  );
  assert_matches!(
    program_test_context
      .banks_client
      .process_transaction(transaction)
      .await,
    Ok(())
  );
}

/// Generates tokens & token-accounts to hold them in the specified numbers.
///
/// Returns a Vec matrix in which each row corresponds to a single token, the first value in the
/// row is the mint account, and the remaining values are token accounts.
async fn create_tokens_and_accounts(
  program_test_context: &mut ProgramTestContext,
  num_tokens: u64,
  num_accounts: u64,
) -> Vec<Vec<Keypair>> {
  let mint_client_vault_accounts = (1..(num_tokens + 1))
    .map(|_| {
      (1..(num_accounts + 2))
        .map(|_| Keypair::new())
        .collect::<Vec<Keypair>>()
    })
    .collect::<Vec<Vec<Keypair>>>();

  // Mint our various tokens & setup accounts.
  for accounts in mint_client_vault_accounts.iter() {
    let mut instructions = Vec::with_capacity(2);
    let mint = &accounts[0]; // First account is always mint
    instructions.push(system_instruction::create_account(
      &program_test_context.payer.pubkey(),
      &mint.pubkey(),
      1.max(Rent::default().minimum_balance(spl_token::state::Mint::LEN)),
      spl_token::state::Mint::LEN as u64,
      &spl_token::id(),
    ));
    instructions.push(
      spl_token::instruction::initialize_mint(
        &spl_token::id(),
        &mint.pubkey(),
        &program_test_context.payer.pubkey(),
        None, // Freeze authority
        6,    // decimals
      )
      .unwrap(),
    );
    let mut transaction =
      Transaction::new_with_payer(&instructions, Some(&program_test_context.payer.pubkey()));
    transaction.sign(
      &[&program_test_context.payer, &mint],
      program_test_context.last_blockhash,
    );
    assert_matches!(
      program_test_context
        .banks_client
        .process_transaction(transaction)
        .await,
      Ok(())
    );

    println!("mint: {}", mint.pubkey());

    for token_account in accounts[1..].iter() {
      println!("token_account: {}", token_account.pubkey());
      let mut instructions = Vec::with_capacity(2);
      instructions.push(system_instruction::create_account(
        &program_test_context.payer.pubkey(),
        &token_account.pubkey(),
        1.max(Rent::default().minimum_balance(spl_token::state::Account::LEN)),
        spl_token::state::Account::LEN as u64,
        &spl_token::id(),
      ));
      instructions.push(
        spl_token::instruction::initialize_account(
          &spl_token::id(),
          &token_account.pubkey(),
          &mint.pubkey(),
          &program_test_context.payer.pubkey(),
        )
        .unwrap(),
      );
      // Note: We can only sign with so many signatures at once, so we need to split transactions
      // up quite a
      let mut transaction =
        Transaction::new_with_payer(&instructions, Some(&program_test_context.payer.pubkey()));
      transaction.sign(
        &[&program_test_context.payer, &token_account],
        program_test_context.last_blockhash,
      );
      assert_matches!(
        program_test_context
          .banks_client
          .process_transaction(transaction)
          .await,
        Ok(())
      );
    }
  }
  return mint_client_vault_accounts;
}
