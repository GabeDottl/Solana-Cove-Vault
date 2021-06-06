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
import {
  Token,
  TOKEN_PROGRAM_ID,
  MintLayout,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { getNodeConnection } from "../nodeConnection";
import { VAULT_PROGRAM_ID, createHodlVault, deposit, withdraw, e2e, addLamports } from "../instruction";

test("Test", async (done) => {
  jest.setTimeout(180000);
  const connection = await getNodeConnection();
  const payerAccount = new Keypair(); //Keypair.fromSecretKey(PAYER_SECRET);
  await e2e(connection, payerAccount);

  done();
});
