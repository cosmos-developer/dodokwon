#!/bin/bash

MAX_ATTEMPTS=3
SLEEP_TIME=5

WALLET=$1
CONTRACT_PATH=$2
shift 2

CHAIN_ID="localterra"
RPC="http://localhost:26657"
ACTION="all"

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
        --action)
            ACTION=$2
            shift
            ;;
        --code-id)
            CODE_ID=$2
            shift
            ;;
        --contract-address)
            CONTRACT_ADDR=$2
            ;;
        *)
            ;;
    esac

    shift
done

NODE="--node $RPC"
TXFLAG="$NODE --chain-id $CHAIN_ID --gas-prices 2500000uluna --gas auto --gas-adjustment 1.3 "
WALLET_ADDRESS=$(terrad keys list --output json | jq -c "[ .[] | select( .name == \"$WALLET\") ][0].address")

# 1. Store code
if [ "$ACTION" = "all" ] || [ "$ACTION" = "store" ]; then
    echo -e "\nStoring code..."
    TX=$(terrad tx wasm store $CONTRACT_PATH --from $WALLET $TXFLAG --output json -y | jq -r '.txhash')

    echo "Storing tx hash: $TX"

    attempts=0
    success=false
    while [ $attempts -lt $MAX_ATTEMPTS ] && [ "$success" = false ]; do
        sleep $SLEEP_TIME
        CODE_ID=$(terrad query tx $TX $NODE --output json | jq -r '.logs[0].events[-1].attributes[1].value')

        if [ -n "$CODE_ID" ]; then
            echo "CODE_ID: $CODE_ID"
            success=true
        else
            attempts=$((attempts + 1))
        fi
    done
    if [ "$success" = false ]; then
        echo "Exceeded maximum attempts. Unable to retrieve CODE_ID."
        exit 1
    fi
fi

# 2. Instantiate contract
if [ "$ACTION" = "all" ] || [ "$ACTION" = "instantiate" ]; then
    if [ -z "$CODE_ID" ]; then
        echo "Expected --code-id"
        exit 1
    fi

    if [ -z "$CROWD_SALE_ADDRESS" ]; then
        echo "Expected --crowd-sale-address"
        exit 1
    fi

    echo -e "\n\nInstantiating contract..."
    INITIAL_STATE="{\"name\" : \"MyCoin\", \"symbol\": \"MCO\", \"decimals\": 6, \"initial_balances\": [{\"address\": $WALLET_ADDRESS, \"amount\": \"3000\"}], \"mint\": {\"minter\": $CROWD_SALE_ADDRESS, \"cap\": \"100000}}"
    INSTANTIATE_TX=$(terrad tx wasm instantiate $CODE_ID "$INITIAL_STATE" --from $WALLET $TXFLAG -y --no-admin)

    attempts=0
    success=false
    while [ $attempts -lt $MAX_ATTEMPTS ] && [ "$success" = false ]; do
        sleep $SLEEP_TIME
        CONTRACT_ADDR=$(terrad query wasm list-contract-by-code $CODE_ID $NODE --output json | jq -r '.contracts[0]')

        if [ -n "$CONTRACT_ADDR" ]; then
            echo "CONTRACT_ADDRESS: $CONTRACT_ADDR"
            success=true
        else
            attempts=$((attempts + 1))
        fi
    done
    if [ "$success" = false ]; then
        echo "Exceeded maximum attempts. Unable to retrieve CONTRACT_ADDRESS."
        exit 1
    fi
fi