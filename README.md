## 0. Prepare

```sh
$ cd scripts

$ sudo chmod +x *
```

## 1. Store all contracts

```sh
$ ./01-store-all-contracts.sh <WALLET> \
  --cw20-base ../artifacts/cw20_token.wasm \
  --foundation ../artifacts/foundation.wasm \
  --crowd-sale ../artifacts/crowd_sale.wasm \
  --network testnet
```

## 2. Instantiate contracts

- Having 3 contracts:

  - CW20: DoDoKwan token
  - Crowd sale: sale token
  - Foundation: hold initial balance

- Foundation and Crowd Sale contracts need CW20 address to instantiate, so need to instantiate CW20 first.
- Instantiate CW20 with both of foundation and minter roles assigned to initializer.
- Once foundation and crowd sale contracts are deployed:
  - Transfer initial balance to foundation contract.
  - Update minter role to crowd sale contract.

```sh
$ ./02-predict-contract-addresses.sh <WALLET> --network testnet
```

If an error occurs while instantiating a contract, you can use the `--only-contract` flag with the following values: `cw20-base`, `crowd-sale`, or `foundation`.

## 3. Transfer CW20 to Foundation and Update Minter to Crowd Sale

```sh
$ ./03-transfer-and-update-minter.sh <WALLET> --network testnet
```

## 4. Try execute

- Buyer call mint in crowd sale contract

```sh
$ ./04-try-query-and-execute.sh <BUYER-WALLET> --network testnet \
  --action mint --uluna <AMOUNT>
```

- Add new voter

```sh
$ ./04-try-query-and-execute.sh <CURRENT_VOTER_WALLET> --network testnet \
  --action add-voter --voter <NEW_VOTER_WALLET>
```

- Send cw20

```sh
$ ./04-try-query-and-execute.sh <VOTER_WALLET_1> --network testnet \
  --action send --voter <VOTER_WALLET__2>
```

- Remove existing voter

```sh
$ ./04-try-query-and-execute.sh <CURRENT_VOTER_WALLET> --network testnet \
  --action remove-voter --voter <REMOVE_VOTER_WALLET>
```
