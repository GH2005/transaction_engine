# Assumptions and Inferences
1. Only deposit transactions can be disputed. The description of dispute, resolve and chargeback do not make sense when considering withdrawal transactions, as reversing a withdrawal shouldn't result in another withdrawal.
2. A dispute should fail when there is not enough available funds to move into held.
3. If a transaction is already under dispute, another same dispute should be ignored.
4. There are two choices after a successful dispute: resolve or chargeback. A resolve reverses the dispute, meaning the deposit under dispute was a good one and the funds should remain in the bank, and after the resolve, it can be disputed again. A chargeback says that the deposit under dispute was made wrongly and funds should be returned to the client, and no more dispute is allowed towards this deposit transaction.
5. If a client is locked, all subsequent transactions made by the client should be ignored.
6. A client cannot file disputes, resolves or chargebacks to transactions made by another client.
7. An amount must be positive.

# Test and Run
```
cargo test
cargo run -- sample_input.csv >sample_output.csv
```
stderr will log all errors during the processing.
