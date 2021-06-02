const BufferLayout = require("buffer-layout");
import {
  Connection,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
  SystemProgram,
  Keypair,
  TransactionInstruction,
} from "@solana/web3.js";
import BN = require("bn.js");
import {
  Token,
  TOKEN_PROGRAM_ID,
  MintLayout,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { getNodeConnection } from "../nodeConnection";
import { VAULT_PROGRAM_ID, createHodlVault, deposit, withdraw } from "../instruction";

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

const PAYER_SECRET = Uint8Array.from([
  236, 230, 136, 92, 193, 87, 127, 175, 140, 212, 209, 156, 162, 184, 231, 133,
  51, 134, 84, 142, 167, 122, 179, 178, 243, 106, 14, 147, 54, 39, 94, 91, 150,
  109, 30, 91, 243, 8, 162, 132, 79, 82, 162, 224, 4, 120, 12, 248, 107, 214,
  24, 112, 33, 205, 126, 173, 174, 249, 47, 118, 221, 89, 132, 165,
]);
async function addLamports(
  connection: Connection,
  account: Keypair,
  lamports = 10000000
) {
  if (lamports <= (await connection.getBalance(account.publicKey))) {
    const count = await connection.getBalance(account.publicKey);
    console.log(`${count} lamports held by payer`);
    return account;
  }

  for (let retry = 0; retry < 10; retry++) {
    try {
      await connection.requestAirdrop(account.publicKey, lamports);
      break;
    } catch (e) {
      console.log(`Airdrop failed: ${e}`);
    }
  }

  for (let retry = 0; retry < 10; retry++) {
    await sleep(500);
    if (lamports <= (await connection.getBalance(account.publicKey))) {
      const count = await connection.getBalance(account.publicKey);
      console.log(`${count} lamports held by payer`);
      return account;
    }
    console.log(`Airdrop retry ${retry}`);
  }
  throw new Error(`Airdrop of ${lamports} failed`);
}

test("Test", async (done) => {
  jest.setTimeout(180000);
  const connection = await getNodeConnection();
  const payerAccount = new Keypair(); //Keypair.fromSecretKey(PAYER_SECRET);
  await addLamports(connection, payerAccount);
  console.log("Setup payer account");
  const tokenA = await Token.createMint(
    connection,
    payerAccount,
    payerAccount.publicKey,
    null,
    6,
    TOKEN_PROGRAM_ID
  );
  
  // console.log("Created mints");
  // await addLamports(connection, payerAccount, 10000000);
  const clientTokenAAccountKey = await tokenA.createAccount(
    payerAccount.publicKey
  );
  // const clientTokenlAAccountKey = await tokenlA.createAccount(payerAccount.publicKey);
  const vaultTokenAAccountKey = await tokenA.createAccount(
    payerAccount.publicKey
  );
  // const vaultTokenlAAccountKey = await tokenlA.createAccount(payerAccount.publicKey);
  await addLamports(connection, payerAccount, 10000000);
  await tokenA.mintTo(clientTokenAAccountKey, payerAccount, [], 1000);
  console.log(`Created accounts and sent 1000 tokens to ${clientTokenAAccountKey}.`);
  let account_info = await tokenA.getAccountInfo(clientTokenAAccountKey);
  expect(account_info.amount.toString()).toEqual('1000');
  console.log(`Confirmed balance of 1000 tokens.`);

  const tokenlA = await Token.createMint(connection, payerAccount, payerAccount.publicKey, null, 6, TOKEN_PROGRAM_ID);
  // const vaultTokenlAAccountKey = await tokenlA.createAccount(payerAccount.publicKey);
  const clientTokenlAAccountKey = await tokenlA.createAccount(payerAccount.publicKey);
  // Setup the HODL vault for tokenA
  await addLamports(connection, payerAccount, 10000000);
  await createHodlVault(connection, payerAccount, vaultTokenAAccountKey,
    false // debug_crash
    ).then(
    async (vaultStorageAccount) => {
      console.log("Created hodl vault");
      await deposit(
        connection,
        payerAccount,
        VAULT_PROGRAM_ID,
        vaultStorageAccount,
        clientTokenAAccountKey,
        clientTokenlAAccountKey,
        vaultTokenAAccountKey,
        10,
        false // debug_crash
      ).then(async (_) => {
        console.log("Deposited into vault account", vaultTokenAAccountKey.toBase58());
        await tokenA.getAccountInfo(vaultTokenAAccountKey).then(account_info => {
          console.log("vaultTokenAAccountKey ", vaultTokenAAccountKey.toBase58(), account_info);
          expect(account_info.amount.toString()).toEqual("10");
          console.log("Confirmed vault balance of 10 tokens.");
        });
        await withdraw(
          connection,
          payerAccount,
          VAULT_PROGRAM_ID,
          vaultStorageAccount,
          clientTokenlAAccountKey,
          clientTokenAAccountKey,
          vaultTokenAAccountKey,
          10,
          false // debug_crash
        ).then(async (_) => {
          console.log("Withdrew {} from vault {}", 10, vaultTokenAAccountKey);
          console.log(`Created accounts and sent 1000 tokens to ${clientTokenAAccountKey}.`);
          let account_info = await tokenA.getAccountInfo(vaultTokenAAccountKey);
          expect(account_info.amount.toString()).toEqual('0');
          console.log(`Confirmed vault balance of 0 tokens.`);
        });
      });
    }
  );

  done();
});
