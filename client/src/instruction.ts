const BufferLayout = require("buffer-layout");
const VariantLayout = BufferLayout.getVariantLayout;
import {
  Connection,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
  SystemProgram,
  Keypair,
  TransactionInstruction,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import BN = require("bn.js");
import { Token, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { makeAccount } from "./tests/integration";

export const VAULT_PROGRAM_ID = new PublicKey(
  "9VxcdZKmmL6xwJWZorYnD29tZte5M29XAiKv3ZEW2AJd"
);

// Accounts expected:
// `[signer]` initializer of the lx token account
// `[writeable]` Vault storage account (vault ID)
// `[]` Vault's lX or X token account
// `[]` The llX mint account
// `[]` The strategy program
// `[]` The rent sysvar
// `[]` (Optional) Strategy instance data account
const InitializeVault = 0;
let vault_instruction_layout = {};
vault_instruction_layout["InitializeVault"] = [
  0,
  BufferLayout.struct([
    BufferLayout.u8("instruction_num"),
    // TODO(007): Governance address, strategist address, keeper address.
    // TODO(008): Withdrawal fee.
    // https://github.com/yearn/yearn-vaults/blob/master/contracts/BaseStrategy.sol#L781
    BufferLayout.u8("strategy_program_deposit_instruction_id"),
    BufferLayout.u8("strategy_program_withdraw_instruction_id"),
    BufferLayout.u8("strategy_program_estimate_instruction_id"),
    BufferLayout.u8("hodl"),
  ]),
];

// Deposits a given token into the vault.
//
// Note this API is an implementation of the StrategyInstruction#Deposit instruction.
//
// Accounts expected:
// 1. `[]` SPL Token program
// 2. `[signer]` The source wallet containing X tokens.
// 3. `[]` The target wallet for llX tokens.
// 4+ `[]` Source signers
// 5. `[]` The Vault storage account.
// 6. `[]` The strategy program.
// 7. `[]` (Optional) X SPL account owned by Vault if hodling.
// 8+. `[]` Strategy extra accoounts (see StrategyInstruction#Deposit)
// TODO(009):: Signer pubkeys for multisignature wallets - need signer_num param.
vault_instruction_layout["Deposit"] = [
  1,
  BufferLayout.struct([
    BufferLayout.u8("instruction_num"),
    BufferLayout.nu64("amount"),
  ]),
];

// Withdraws a token from the vault.
//
// Note this API is an implementation of the StrategyInstruction#Withdraw instruction.
//
// Accounts expected:
// 1. `[]` SPL Token program
// 2. `[signer]` Source Wallet for derivative token (lX).
// 3. `[]` Target token (X) wallet target.
// 4+ `[]` Source signers
// 5. `[]` The Vault storage account.
// 6. `[]` The strategy program.
// 7. `[]` (Optional) X SPL account owned by Vault if hodling.
// 8+. `[]` Strategy extra accoounts (see StrategyInstruction#Withdraw)
// TODO(009):: Signer pubkeys for multisignature wallets - need signer_num param.
vault_instruction_layout["Withdraw"] = [
  2,
  BufferLayout.struct([
    BufferLayout.u8("instruction_num"),
    BufferLayout.nu64("amount"), // # of derivative tokens.
  ]),
];

// Estimates the underlying value of the vault in its native asset.
//
// This instruction stores its results in a temporary account using the Shared Memory program.
// https://spl.solana.com/shared-memory
//
// Accounts expected:
// 1. `[]` Shared Memory program
// 1. `[]` Shared memory output
// 2. `[]` The Vault storage account.
// 3. `[]` (Optional) X SPL account owned by Vault if hodling.
// 4+ `[*]` Strategy extra accounts - any additional accounts required by strategy
vault_instruction_layout["EstimateValue"] = [
  3,
  BufferLayout.struct([BufferLayout.u8("instruction_num")]),
];

// A helper utility which functions similarly to the (unlaunched) Shared Memory program.
//
// Data is read directly from the account memory.
vault_instruction_layout["WriteData"] = [
  4,
  BufferLayout.struct([BufferLayout.u8("instruction_num")]),
];

export async function createHodlVault(
  connection: Connection,
  payer_account: Keypair,
  token_x: Token
) {
  console.log("Creating HODL vault");
  // let vault_token_account_owner = makeAccount(connection, payer_account, Token, PROGRAM_ID);
  const token_vault_derivative = await Token.createMint(
    connection,
    payer_account,
    payer_account.publicKey,
    null,
    6,
    TOKEN_PROGRAM_ID
  );

  // const vault_vault_token_account = await token_vault_derivative.createAccount(payer_account.publicKey);
  let vault_storage_account = await makeAccount(
    connection,
    payer_account,
    1 + 1 + 32 + 32 + 8 + 32 + 1 + 1 + 1 + 36,
    VAULT_PROGRAM_ID
  );
  console.log("vault_storage_account ", vault_storage_account);
  const vault_token_account = await token_x.createAccount(
    payer_account.publicKey
  );
  // let vault_token_account = makeAccount(connection, payer_account, numBytes, PROGRAM_ID);
  // let llx_token_mint_id = makeAccount(connection, payer_account, numBytes, PROGRAM_ID);
  let instruction = initializeVaultInstruction(
    payer_account.publicKey, // TODO: Separate owner
    vault_storage_account,
    vault_token_account,
    token_vault_derivative.publicKey,
    TOKEN_PROGRAM_ID,
    VAULT_PROGRAM_ID,
    1,
    2,
    3,
    true
  );
  let transaction = new Transaction();
  transaction.add(instruction);
  console.log("Sending instruction to create HODL vault");
  await sendAndConfirmTransaction(connection, transaction, [payer_account]);
  console.log(
    "vault_storage_account ",
    vault_storage_account.toBase58(),
    connection.getAccountInfo(vault_storage_account)
  );
  console.log(
    "vault_token_account ",
    vault_token_account.toBase58(),
    connection.getAccountInfo(vault_token_account)
  );
  console.log("Created hodl vault");
  let account_info = await connection.getAccountInfo(vault_storage_account);
  console.log("data_account ", vault_storage_account.toBase58(), account_info);
  return vault_storage_account;
}

// Note: These instructions mirror instruction.rs.
function initializeVaultInstruction(
  vault_token_account_owner: PublicKey,
  vault_storage_account: PublicKey,
  vault_token_account: PublicKey,
  llx_token_mint_id: PublicKey,
  token_program: PublicKey,
  strategy_program: PublicKey,
  strategy_program_deposit_instruction_id: number,
  strategy_program_withdraw_instruction_id: number,
  strategy_program_estimate_value_instruction_id: number,
  hodl: boolean
) {
  console.log(
    "vault_token_account_owner ",
    vault_token_account_owner.toBase58()
  );
  console.log("vault_storage_account ", vault_storage_account.toBase58());
  console.log("vault_token_account ", vault_token_account.toBase58());
  console.log("llx_token_mint_id ", llx_token_mint_id.toBase58());
  console.log("token_program ", token_program.toBase58());
  console.log("strategy_program ", strategy_program.toBase58());
  const accounts = [
    { pubkey: vault_token_account_owner, isSigner: true, isWritable: false },
    { pubkey: vault_storage_account, isSigner: false, isWritable: true },
    { pubkey: vault_token_account, isSigner: false, isWritable: true },
    { pubkey: llx_token_mint_id, isSigner: false, isWritable: true },
    { pubkey: token_program, isSigner: false, isWritable: false },
    { pubkey: strategy_program, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
  ];
  let data = {
    instruction: InitializeVault,
    strategy_program_deposit_instruction_id:
      strategy_program_deposit_instruction_id,
    strategy_program_withdraw_instruction_id:
      strategy_program_withdraw_instruction_id,
    strategy_program_estimate_instruction_id:
      strategy_program_estimate_value_instruction_id,
    hodl: hodl,
  };
  console.log(
    "instruction layout size {}",
    vault_instruction_layout["InitializeVault"][1].getSpan(data)
  );
  return new TransactionInstruction({
    keys: accounts,
    data: encodeInstructionData(
      data,
      vault_instruction_layout["InitializeVault"][1]
    ),
    programId: VAULT_PROGRAM_ID,
  });
}

export async function deposit(
  connection: Connection,
  payer_account: Keypair,
  strategy_program: PublicKey,
  vault_account: PublicKey,
  client_x_token_account: PublicKey,
  vault_x_token_account: PublicKey,
  amount: number
): Promise<void> {
  console.log("vault_account ", vault_account.toBase58());
  let transaction = new Transaction();
  console.log("payer_account {}", payer_account.publicKey.toBase58());
  console.log("vault_account {}", vault_account.toBase58());
  transaction.add(
    depositInstruction(
      VAULT_PROGRAM_ID,
      TOKEN_PROGRAM_ID,
      client_x_token_account,
      vault_x_token_account,
      [
        { isWritable: false, pubkey: payer_account.publicKey, isSigner: true },
        { isWritable: true, pubkey: vault_account, isSigner: false },
        { isWritable: false, pubkey: strategy_program, isSigner: false },
        { isWritable: false, pubkey: vault_x_token_account, isSigner: false },
      ],
      amount
    )
  );
  sendAndConfirmTransaction(connection, transaction, [payer_account]);
}

function depositInstruction(
  vault_program_id: PublicKey,
  token_program_id: PublicKey,
  source_pubkey: PublicKey,
  target_pubkey: PublicKey,
  additional_account_metas: {
    isWritable: boolean;
    pubkey: PublicKey;
    isSigner: boolean;
  }[],
  amount: number
) {
  console.log("vault_program_id {}", vault_program_id.toBase58());
  console.log("token_program_id {}", token_program_id.toBase58());
  console.log("source_pubkey {}", source_pubkey.toBase58());
  console.log("target_pubkey {}", target_pubkey.toBase58());
  let data = {
    instruction_num: vault_instruction_layout["Deposit"][0],
    amount,
  };
  let instructionData = encodeInstructionData(
    data,
    vault_instruction_layout["Deposit"][1]
  );
  return createTransferInstruction(
    instructionData,
    vault_program_id,
    token_program_id,
    source_pubkey,
    target_pubkey,
    additional_account_metas
  );
}

function withdrawInstruction(
  vault_program_id: PublicKey,
  token_program_id: PublicKey,
  source_pubkey: PublicKey,
  target_pubkey: PublicKey,
  additional_account_metas: {
    isWritable: boolean;
    pubkey: PublicKey;
    isSigner: boolean;
  }[],
  amount: number
) {
  console.log("vault_program_id {}", vault_program_id.toBase58());
  console.log("token_program_id {}", token_program_id.toBase58());
  console.log("source_pubkey {}", source_pubkey.toBase58());
  console.log("target_pubkey {}", target_pubkey.toBase58());
  let data = {
    instruction_num: vault_instruction_layout["Deposit"][0],
    amount,
  };
  let instructionData = encodeInstructionData(
    data,
    vault_instruction_layout["Deposit"][1]
  );
  return createTransferInstruction(
    instructionData,
    vault_program_id,
    token_program_id,
    source_pubkey,
    target_pubkey,
    additional_account_metas
  );
}
function createTransferInstruction(
  data: Buffer,
  program_id: PublicKey,
  token_program_id: PublicKey,
  source_pubkey: PublicKey,
  target_pubkey: PublicKey,
  additional_account_metas: {
    isWritable: boolean;
    pubkey: PublicKey;
    isSigner: boolean;
  }[]
) {
  let accounts: {
    isWritable: boolean;
    pubkey: PublicKey;
    isSigner: boolean;
  }[] = [
    { isWritable: false, pubkey: token_program_id, isSigner: false },
    { isWritable: true, pubkey: source_pubkey, isSigner: false },
    { isWritable: true, pubkey: target_pubkey, isSigner: false },
  ].concat(additional_account_metas);
  // accounts = accounts;
  return new TransactionInstruction({
    keys: accounts,
    data,
    programId: VAULT_PROGRAM_ID,
  });
}

function encodeInstructionData(
  instruction: Object,
  layout: typeof BufferLayout
) {
  // const instructionMaxSpan = Math.max(...Object.values(layout.registry).map((r: typeof BufferLayout) => r.span));
  const b = Buffer.alloc(layout.getSpan(instruction));
  const span = layout.encode(instruction, b);
  return b.slice(0, span);
}

export async function createAccount(
  connection: Connection,
  payer_account: Keypair,
  numBytes: number,
  programId: PublicKey
) {
  const dataAccount = new Keypair();
  const rentExemption = await connection.getMinimumBalanceForRentExemption(
    numBytes
  );
  const transaction = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: payer_account.publicKey,
      newAccountPubkey: dataAccount.publicKey,
      lamports: rentExemption,
      space: numBytes,
      programId: programId,
    })
  );

  await sendAndConfirmTransaction(connection, transaction, [
    payer_account,
    dataAccount,
  ]);

  return dataAccount.publicKey;
}
