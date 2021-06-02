use crate::error::{VaultError, VaultError::InvalidInstruction};
use solana_program::program_error::ProgramError;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    msg,
    program_option::COption,
    pubkey::Pubkey,
    sysvar,
};
use strategy_api::strategy_instruction::{create_estimate_value, create_transfer};

use std::convert::TryInto;
use std::mem::size_of;

pub enum VaultInstruction {
    /// Creates a defi Vault.
    ///
    /// Vaults are a mechanism for building investment strategies around an underlying token asset
    /// (X). Vaults are designed to be highly composable they just hold the underlying strategy's
    /// asset (lX) or the token asset (X) in the case of HODL Vaults. In exchange for depositing
    /// X tokens to a vault, a new llX derivative token from the vault is minted proportional to
    /// the relative value added. If you increase the X assets held by X%, you get X% of the llX
    /// supply at the time of deposit. When withdrawing X tokens via your llX tokens, clients are
    /// charged a fixed-rate, configurable fee out of the returned assets.
    ///
    /// The interaction with a Vault looks like the following:
    ///
    /// Deposit:
    ///   User sends X tokens to a Vault, the Vault sends the tokens to the strategy (or HODL) and
    ///   gets back an lX token, which it stores, and then mints a corresponding llX token which it
    ///   gives to the user.
    /// Withdraw:
    ///   User sends llX tokens to a Vault, the Vault burns the llX tokens and sends the
    ///   corresponding percentage of lX tokens to the strategy and gets back X tokens, which it
    ///   forwards to the user, minus a fee.
    ///
    /// Strategies should be contained within a single program and should implement the
    /// StrategyInstruction interface below. If a Strategy requires additional data, it can specify
    /// it in a data account which will be included in calls to the strategy instance. Extra
    /// accounts passed to Deposit/Withdraw functions will be passed along to strategies.
    ///
    /// TODO(006): Consider reusing X & lX token accounts depending on whether or not the Vault is
    /// a HODL vault. Also, drop the strategy_data_account - it's not needed.
    ///
    /// Accounts expected:
    /// // TODO(014): Separate token owner from mint owner.
    /// `[signer]` Vault token account owner & mint owner
    /// `[writeable]` Vault storage account (vault ID)
    /// `[]` Vault's lX token account or X token account if hodling  
    /// `[]` The llX mint account
    /// `[]` The strategy program
    /// `[]` The rent sysvar
    /// `[]` (Optional) Strategy instance data account
    InitializeVault {
        // TODO(007): Governance address, strategist address, keeper address.
        // TODO(008): Withdrawal fee.
        // https://github.com/yearn/yearn-vaults/blob/master/contracts/BaseStrategy.sol#L781
        strategy_program_deposit_instruction_id: u8,
        strategy_program_withdraw_instruction_id: u8,
        strategy_program_estimate_instruction_id: u8,
        hodl: bool,
        debug_crash: bool,
    },

    /// Deposits a given token into the vault.
    ///
    /// Note this API is an implementation of the StrategyInstruction#Deposit instruction.
    ///
    /// Accounts expected:
    /// 1. `[]` SPL Token program
    /// 2. `[signer]` The source wallet containing X tokens.
    /// 3. `[]` The target wallet for llX tokens.
    /// 4+ `[]` Source signers
    /// 5. `[]` The Vault storage account.
    /// 6. `[]` The strategy program.
    /// 7. `[]` (Optional) X SPL account owned by Vault if hodling.
    /// 8+. `[]` Strategy extra accoounts (see StrategyInstruction#Deposit)
    /// TODO(009):: Signer pubkeys for multisignature wallets - need signer_num param.
    Deposit { amount: u64, debug_crash: bool },

    /// Withdraws a token from the vault.
    ///
    /// Note this API is an implementation of the StrategyInstruction#Withdraw instruction.
    ///
    /// Accounts expected:
    /// 1. `[]` SPL Token program
    /// 2. `[signer]` Source Wallet for derivative token (lX).
    /// 3. `[]` Target token (X) wallet target.
    /// 4+ `[]` Source signers
    /// 5. `[]` The Vault storage account.
    /// 6. `[]` The strategy program.
    /// 7. `[]` (Optional) X SPL account owned by Vault if hodling.
    /// 8+. `[]` Strategy extra accoounts (see StrategyInstruction#Withdraw)
    /// TODO(009):: Signer pubkeys for multisignature wallets - need signer_num param.
    Withdraw {
        amount: u64, // # of derivative tokens.
        debug_crash: bool,
    },

    /// Estimates the underlying value of the vault in its native asset.
    ///
    /// This instruction stores its results in a temporary account using the Shared Memory program.
    /// https://spl.solana.com/shared-memory
    ///
    /// Accounts expected:
    /// 1. `[]` Shared Memory program
    /// 1. `[]` Shared memory output
    /// 2. `[]` The Vault storage account.
    /// 3. `[]` (Optional) X SPL account owned by Vault if hodling.
    /// 4+ `[*]` Strategy extra accounts - any additional accounts required by strategy
    EstimateValue { debug_crash: bool },

    /// A helper utility which functions similarly to the (unlaunched) Shared Memory program.
    ///
    /// Data is read directly from the account memory.
    WriteData {
        debug_crash: bool, // data: &'a [u8]
    },
}
pub const CRASH_FLAG: u8 = 64;

impl VaultInstruction {
    /// Unpacks a byte buffer into a [VaultInstruction](enum.VaultInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (tag_raw, rest) = input.split_first().ok_or(InvalidInstruction)?;
        let debug_crash: bool = *tag_raw >= CRASH_FLAG;
        let tag = if *tag_raw >= CRASH_FLAG {
            *tag_raw - CRASH_FLAG
        } else {
            *tag_raw
        };
        msg!("Debug crash: {} {} {}", debug_crash, tag_raw, tag);
        Ok(match tag {
            0 => {
                let hodl = *rest.get(0).unwrap();
                let strategy_program_deposit_instruction_id = *rest.get(1).unwrap();
                let strategy_program_withdraw_instruction_id = *rest.get(2).unwrap();
                let strategy_program_estimate_instruction_id = *rest.get(3).unwrap();
                Self::InitializeVault {
                    hodl: if hodl == 1 { true } else { false },
                    strategy_program_deposit_instruction_id,
                    strategy_program_withdraw_instruction_id,
                    strategy_program_estimate_instruction_id,
                    debug_crash,
                }
            }
            1 | 2 => {
                let amount = rest
                    .get(..8)
                    .and_then(|slice| slice.try_into().ok())
                    .map(u64::from_le_bytes)
                    .ok_or(InvalidInstruction)?;
                match tag {
                    1 => Self::Deposit {
                        amount,
                        debug_crash,
                    },
                    2 => Self::Withdraw {
                        amount,
                        debug_crash,
                    },
                    _ => return Err(VaultError::InvalidInstruction.into()),
                }
            }
            3 => Self::EstimateValue { debug_crash },
            4 => {
                // Data unpacked separately.
                Self::WriteData { debug_crash }
            }
            _ => return Err(VaultError::InvalidInstruction.into()),
        })
    }

    fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match self {
            &Self::InitializeVault {
                hodl,
                strategy_program_deposit_instruction_id,
                strategy_program_withdraw_instruction_id,
                strategy_program_estimate_instruction_id,
                debug_crash,
            } => {
                buf.push(0 + (if debug_crash { CRASH_FLAG } else { 0 }));
                buf.push(hodl as u8);
                buf.push(strategy_program_deposit_instruction_id);
                buf.push(strategy_program_withdraw_instruction_id);
                buf.push(strategy_program_estimate_instruction_id);
            }
            &Self::Deposit {
                amount,
                debug_crash,
            } => {
                buf.push(1 + (if debug_crash { CRASH_FLAG } else { 0 }));
                buf.extend_from_slice(&amount.to_le_bytes());
            }

            &Self::Withdraw {
                amount,
                debug_crash,
            } => {
                buf.push(2 + (if debug_crash { CRASH_FLAG } else { 0 }));
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            &Self::EstimateValue { debug_crash } => {
                buf.push(3 + (if debug_crash { CRASH_FLAG } else { 0 }));
            }
            // Data packed separately.
            &Self::WriteData { debug_crash } => {
                buf.push(4 + (if debug_crash { CRASH_FLAG } else { 0 }));
            }
        }
        buf
    }

    pub fn write_data(
        vault_program_id: &Pubkey,
        shared_memory_account: &Pubkey,
        data: &[u8],
    ) -> Result<Instruction, ProgramError> {
        let accounts = vec![AccountMeta::new(*shared_memory_account, false)];
        let mut instruction_data = Self::WriteData { debug_crash: false }.pack();
        instruction_data.extend(data);
        Ok(Instruction {
            program_id: *vault_program_id,
            accounts,
            data: instruction_data,
        })
    }

    pub fn initialize_vault(
        vault_program_id: &Pubkey,
        initializer: &Pubkey,
        vault_storage_account: &Pubkey,
        vault_token_account: &Pubkey,
        llx_token_mint_id: &Pubkey,
        token_program: &Pubkey,
        strategy_program: &Pubkey,
        hodl: bool,
        strategy_program_deposit_instruction_id: u8,
        strategy_program_withdraw_instruction_id: u8,
        strategy_program_estimate_instruction_id: u8,
    ) -> Result<Instruction, ProgramError> {
        let accounts = vec![
            AccountMeta::new_readonly(*initializer, true),
            AccountMeta::new(*vault_storage_account, false),
            AccountMeta::new(*vault_token_account, false),
            AccountMeta::new(*llx_token_mint_id, false),
            AccountMeta::new_readonly(*token_program, false),
            AccountMeta::new_readonly(*strategy_program, false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
        ];
        let data = VaultInstruction::InitializeVault {
            strategy_program_deposit_instruction_id,
            strategy_program_withdraw_instruction_id,
            strategy_program_estimate_instruction_id,
            hodl,
            debug_crash: false,
        }
        .pack();
        Ok(Instruction {
            program_id: *vault_program_id,
            accounts,
            data,
        })
    }

    pub fn deposit(
        vault_program_id: &Pubkey,
        token_program_id: &Pubkey,
        client_x_token_account: &Pubkey,
        client_lx_token_account: &Pubkey,
        additional_account_metas: Vec<AccountMeta>,
        amount: u64,
    ) -> Result<Instruction, ProgramError> {
        return create_transfer(
            Self::Deposit {
                amount,
                debug_crash: false,
            }
            .pack(),
            vault_program_id,
            token_program_id,
            client_x_token_account,
            client_lx_token_account,
            additional_account_metas,
        );
    }

    pub fn withdraw(
        vault_program_id: &Pubkey,
        token_program_id: &Pubkey,
        client_lx_token_account: &Pubkey,
        client_x_token_account: &Pubkey,
        additional_account_metas: Vec<AccountMeta>,
        amount: u64,
    ) -> Result<Instruction, ProgramError> {
        return create_transfer(
            Self::Withdraw {
                amount,
                debug_crash: false,
            }
            .pack(),
            vault_program_id,
            token_program_id,
            client_lx_token_account,
            client_x_token_account,
            additional_account_metas,
        );
    }

    pub fn estimate_value(
        program_id: &Pubkey,
        vault_program_id: &Pubkey,
        shared_memory_account: &Pubkey,
        additional_account_metas: Vec<AccountMeta>,
    ) -> Result<Instruction, ProgramError> {
        return create_estimate_value(
            Self::EstimateValue { debug_crash: false }.pack(),
            program_id,
            vault_program_id,
            shared_memory_account,
            additional_account_metas,
        );
    }
}
