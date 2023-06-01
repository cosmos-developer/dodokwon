#!/bin/bash

WALLET=$1
shift 1

CHAIN_ID="localterra"
RPC="http://localhost:26657"

while [[ $# -gt 0 ]]; do
    key="$1"

    case $key in
        --network)
            if [ "$2" = "testnet" ]; then
                CHAIN_ID="bajor-1"
                RPC="http://85.214.56.241:26657"
            fi
            shift
            ;;
        *)
            ;;
    esac

    shift
done

NODE="--node $RPC"
TXFLAG="$NODE --chain-id $CHAIN_ID --gas-prices 2500000uluna --gas-adjustment 1.3"
WALLET_ADDRESS=$(terrad keys list --output json | jq -r "[ .[] | select( .name == \"$WALLET\") ][0].address")

MAX_ATTEMPTS=2
SLEEP_TIME=5

CW20_BASE_ADDRESS=$(cat ./contract-addresses/cw20-base-address.txt)
FOUNDATION_ADDRESS=$(cat ./contract-addresses/foundation-address.txt)
CROWD_SALE_ADDRESS=$(cat ./contract-addresses/crowd-sale-address.txt)

# Transfer initial balance to the wallet address
echo -e "\n========== Transfer initial balance to the Foundation contract =========="
QUERY_WALLET_BALANCE_ARGS="{\"balance\":{\"address\":\"$WALLET_ADDRESS\"}}" 
INITIAL_INITIALIZER_BALANCE=$(terrad query wasm contract-state smart $CW20_BASE_ADDRESS $QUERY_WALLET_BALANCE_ARGS $NODE --output json | jq -r .data.balance)
echo -e "\nInitial Balance of initializer: $INITIAL_INITIALIZER_BALANCE"
QUERY_FOUNDATION_BALANCE_ARGS="{\"balance\":{\"address\":\"$FOUNDATION_ADDRESS\"}}" 
INITIAL_FOUNDATION_BALANCE=$(terrad query wasm contract-state smart $CW20_BASE_ADDRESS $QUERY_FOUNDATION_BALANCE_ARGS $NODE --output json | jq -r .data.balance)
echo -e "Initial Balance of foundation: $INITIAL_FOUNDATION_BALANCE"


TRANSFER_ARGS="{\"transfer\":{\"recipient\":\"$FOUNDATION_ADDRESS\",\"amount\":\"$INITIAL_INITIALIZER_BALANCE\"}}"
TRANSFER=$(terrad tx wasm execute $CW20_BASE_ADDRESS $TRANSFER_ARGS --from $WALLET $TXFLAG -y --output json)
TRANSFER_TX_HASH=$(echo $TRANSFER | jq -r .txhash)
echo -e "\nTransfer tx hash: $TRANSFER_TX_HASH"

QUERY_WALLET_BALANCE_ARGS="{\"balance\":{\"address\":\"$WALLET_ADDRESS\"}}" 
AFTER_TRANSFER_INITIALIZER_BALANCE=$(terrad query wasm contract-state smart $CW20_BASE_ADDRESS $QUERY_WALLET_BALANCE_ARGS $NODE --output json | jq -r .data.balance)
echo -e "\nAfter transfer Balance of initializer: $AFTER_TRANSFER_INITIALIZER_BALANCE"
QUERY_FOUNDATION_BALANCE_ARGS="{\"balance\":{\"address\":\"$FOUNDATION_ADDRESS\"}}" 
AFTER_TRANSFER_FOUNDATION_BALANCE=$(terrad query wasm contract-state smart $CW20_BASE_ADDRESS $QUERY_FOUNDATION_BALANCE_ARGS $NODE --output json | jq -r .data.balance)
echo -e "After transfer Balance of foundation: $AFTER_TRANSFER_FOUNDATION_BALANCE"


# Update minter
echo -e "\n\n========== Update minter =========="
echo -e "\nInitilizer: $WALLET_ADDRESS"
echo -e "Foundation contract: $FOUNDATION_ADDRESS"

QUERY_MINTER_ARGS="{\"minter\":{}}" 
MINTER_INFO=$(terrad query wasm contract-state smart $CW20_BASE_ADDRESS $QUERY_MINTER_ARGS $NODE --output json | jq -r .data )
echo -e "\nInitial Minter info: $MINTER_INFO"

UPDATE_MINTER_ARGS="{\"update_minter\":{\"new_minter\":\"$FOUNDATION_ADDRESS\"}}"
UPDATE_MINTER=$(terrad tx wasm execute $CW20_BASE_ADDRESS $UPDATE_MINTER_ARGS --from $WALLET $TXFLAG -y --output json)
UPDATE_MINTER_TX_HASH=$(echo $UPDATE_MINTER | jq -r .txhash)
echo -e "\nUpdate minter tx hash: $UPDATE_MINTER_TX_HASH"

QUERY_MINTER_ARGS="{\"minter\":{}}"
MINTER_INFO=$(terrad query wasm contract-state smart $CW20_BASE_ADDRESS $QUERY_MINTER_ARGS $NODE --output json | jq -r .data )
echo -e "\nAfter update Minter info: $MINTER_INFO"

