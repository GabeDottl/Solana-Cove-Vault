# Cove
Cove is a platform for Laguna (Solana) Vaults.

See src/instructions.rs for the API.

Within Yearn, the derivative of a X token would be yX - within Laguna Finance, we us lX. llX is used
for 2nd order derivatives.

Laguna Vaults are intended to be combined to form directed, acyclic investment graphs of arbitrary
complexity. Fees may be charged and routed at any level of the graph and, in the future, the graph
will be able to react to arbitrary events.

The core benefit of Laguna Vaults are that they mint & distribute a derivative token to users when
depositing proportional to a best-estimate of their contribution to the current underlying value.
This makes it trivial, for example, to create arbitrary wrapper-tokens (like stETH, wETH).

## TODO
TODO(XXX) corresponds to a TODO location in-code.

* Expand design documentation - segmentation of signatures across strategies, token movement.
* Add Peek function to strategy to see underlying value.
* TODO(001): Grant prportional lX tokens when depositing
* TODO(002): Charge lX tokens when withdrawing
* Add Multplexer for splitting tokens across multiple strategies (e.g. hodl & other)
* TODO(008): Add fee support
* TODO(009): Allow multisig client wallets (i.e. support multiple signers)
* Add reporting for calculating yield
* TODO(007): Add support for governance? Might implement above & separate
* Add Tend API for triggering harvesting (or other logic) across the graph on a periodic basis
* Unit tests
* Expand functional tests to include bad cases
* Security audit
* More example vaults
* Cleanup / merge the various Deposits & Withdraw logic
* TODO(006): Maybe refactor initialize_vault API
* TODO(010): Refactor StrategyInstruction to reduce duplicate logic with Vault.
* TODO(011): Remove dev logs and/or gate them appropriately.
* TODO(012): Calculate last_estimated_value dynamically & return with Shared Memory program or a
             similar service. Shared Memory hasn't yet been launched.
* TODO(Security): Fix vulnerabilities.
* TODO(013): Add account metas.
* TODO: Split strategy_api into its own separate, public crate & repo.
* TODO(014): Separate token owner from mint owner.
* TODO: Cove version of https://yearn-hub.vercel.app/
* TODO: Cove version of https://yearn.science/
* TODO: Cleanup - fix snake_case in TS files to be proper camelCase.
* TODO: Add production flags to strip message printing & ignore debug_crash flag.
* TODO: Shift as much program setup logic outside of the Program Instructions as possible - instead, just verify authority et al.

### Environment Setup
1. Install Rust from https://rustup.rs/
2. Install Solana v1.6.2 or later from https://docs.solana.com/cli/install-solana-cli-tools#use-solanas-install-tool


### Build and test the program compiled for BPF
```
$ cargo build-bpf
$ cargo test-bpf
```
cd ~/code/laguna/cove && deploy.sh devnet && cd client && yarn test

gilgameshcoder note: i am gabedottl
