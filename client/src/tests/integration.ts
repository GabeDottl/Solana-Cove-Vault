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

// Load .env
const dotenv = require('dotenv');
dotenv.config();
console.log(`${process.env}`);
console.log(`${process.env.TEST}`);

// Assert that .env loaded correctly
var assert = require('assert');
assert(Number(process.env.TEST) == 2);
console.log("loaded .env");

test("Test", async (done) => {
  jest.setTimeout(180000);
  const connection = await getNodeConnection();
  // console.log(`${process.env}`);
  console.log(`DEVNET_WALLET ${process.env.DEVNET_WALLET}`);
  
  var fs = require('fs');
  let fileData = fs.readFileSync(process.env.DEVNET_WALLET);
  console.log(`fileData ${fileData}`);
  fileData = String(fileData).slice(1,-1)
  console.log(`fileData ${fileData}`);
  let int_array = fileData.split(',').map(function(item) {
    return parseInt(item, 10);
  });
  int_array = Uint8Array.from(int_array); // .slice(32,64);
  console.log(`int_array: ${int_array}`);
  // TODO: Renenable
  const payerAccount = Keypair.fromSecretKey(int_array);
  await e2e(connection, payerAccount);

  // function getByteArray(filePath){
  //     let result = []
  //     for (var i = 0; i < fileData.length; i+=2)
  //       result.push('0x'+fileData[i]+''+fileData[i+1])
  //     return result;
  // }
  
  // dotenv.config({})
  // result = 
  // console.log(getByteArray(''))

  

  done();
});
