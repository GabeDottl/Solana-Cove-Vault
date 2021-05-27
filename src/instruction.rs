use crate::error::{VaultError, VaultError::InvalidInstruction};
use strategy_api::strategy_instruction::{create_estimate_value, create_transfer};
use solana_program::program_error::ProgramError;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_option::COption,
    pubkey::Pubkey,
    sysvar,
};

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
    /// `[signer]` initializer of the lx token account
    /// `[writeable]` Vault storage account (vault ID)
    /// `[]` lX token wallet account
    /// `[]` The llX token mint account
    /// `[]` The strategy program
    /// `[]` The rent sysvar
    /// `[]` (Optional) Strategy instance data account
    /// `[]` (Optional) X token account if hodling
    InitializeVault {
        // TODO(007): Governance address, strategist address, keeper address.
        // TODO(008): Withdrawal fee.
        // https://github.com/yearn/yearn-vaults/blob/master/contracts/BaseStrategy.sol#L781
        strategy_program_deposit_instruction_id: u8,
        strategy_program_withdraw_instruction_id: u8,
        strategy_program_estimate_instruction_id: u8,
        hodl: bool,
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
    Deposit { amount: u64 },

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
    EstimateValue {},

    /// A helper utility which functions similarly to the (unlaunched) Shared Memory program.
    ///
    /// Data is read directly from the account memory.
    WriteData {
        // data: &'a [u8]
    },
}

impl VaultInstruction {
    /// Unpacks a byte buffer into a [VaultInstruction](enum.VaultInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (tag, rest) = input.split_first().ok_or(InvalidInstruction)?;

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
                }
            }
            1 | 2 => {
                let amount = rest
                    .get(..8)
                    .and_then(|slice| slice.try_into().ok())
                    .map(u64::from_le_bytes)
                    .ok_or(InvalidInstruction)?;
                match tag {
                    1 => Self::Deposit { amount },
                    2 => Self::Withdraw { amount },
                    _ => return Err(VaultError::InvalidInstruction.into()),
                }
            }
            3 => Self::EstimateValue {},
            4 => {
                // Data unpacked separately.
                Self::WriteData {}
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
            } => {
                buf.push(0);
                buf.push(hodl as u8);
                buf.push(strategy_program_deposit_instruction_id);
                buf.push(strategy_program_withdraw_instruction_id);
                buf.push(strategy_program_estimate_instruction_id);
            }
            &Self::Deposit { amount } => {
                buf.push(1);
                buf.extend_from_slice(&amount.to_le_bytes());
            }

            &Self::Withdraw { amount } => {
                buf.push(2);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            &Self::EstimateValue {} => {
                buf.push(3);
            }
            // Data packed separately.
            &Self::WriteData {} => {
                buf.push(4);
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
        let mut instruction_data = Self::WriteData {}.pack();
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
        lx_token_account: &Pubkey,
        llx_token_mint_id: &Pubkey,
        token_program: &Pubkey,
        strategy_program: &Pubkey,
        hodl: bool,
        x_token_account: COption<Pubkey>,
        strategy_program_deposit_instruction_id: u8,
        strategy_program_withdraw_instruction_id: u8,
        strategy_program_estimate_instruction_id: u8,
    ) -> Result<Instruction, ProgramError> {
        let mut accounts = vec![
            AccountMeta::new_readonly(*initializer, true),
            AccountMeta::new(*vault_storage_account, false),
            AccountMeta::new(*lx_token_account, false),
            AccountMeta::new(*llx_token_mint_id, false),
            AccountMeta::new_readonly(*token_program, false),
            AccountMeta::new_readonly(*strategy_program, false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
        ];
        assert_eq!(hodl, x_token_account.is_some());
        if hodl {
            accounts.push(AccountMeta::new(x_token_account.unwrap(), false));
        }
        let data = VaultInstruction::InitializeVault {
            hodl,
            strategy_program_deposit_instruction_id,
            strategy_program_withdraw_instruction_id,
            strategy_program_estimate_instruction_id,
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
        source_pubkey: &Pubkey,
        target_pubkey: &Pubkey,
        additional_account_metas: Vec<AccountMeta>,
        amount: u64,
    ) -> Result<Instruction, ProgramError> {
        return create_transfer(
            Self::Deposit { amount }.pack(),
            vault_program_id,
            token_program_id,
            source_pubkey,
            target_pubkey,
            additional_account_metas,
        );
    }

    pub fn withdraw(
        vault_program_id: &Pubkey,
        token_program_id: &Pubkey,
        source_pubkey: &Pubkey,
        target_pubkey: &Pubkey,
        additional_account_metas: Vec<AccountMeta>,
        amount: u64,
    ) -> Result<Instruction, ProgramError> {
        return create_transfer(
            Self::Withdraw { amount }.pack(),
            vault_program_id,
            token_program_id,
            source_pubkey,
            target_pubkey,
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
            Self::EstimateValue {}.pack(),
            program_id,
            vault_program_id,
            shared_memory_account,
            additional_account_metas,
        )
    }
}
