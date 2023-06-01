### 0. Prepare

```sh
$ cd scripts

$ sudo chmod +x *
```

### 1. Store all contracts

```sh
$ ./01-store-all-contracts.sh <WALLET> \
  --cw20-base ../artifacts/cw20_token.wasm \
  --foundation ../artifacts/foundation.wasm \
  --crowd-sale ../artifacts/crowd_sale.wasm \
  --network testnet
```

### 2. Instantiate CW20 base

- Instantiate CW20 base with both of foundation and minter roles assigned to caller.
- Once foundation and crowd sale contracts are deployed:
  - Transfer initial balance to foundation contract.
  - Update minter role to crowd sale contract.

```sh
$ ./02-predict-contract-addresses.sh <WALLET> --network testnet
```

If an error occurs while instantiating a contract, you can use the `--only-contract` flag with the following values: `cw20-base`, `crowd-sale`, or `foundation`.
