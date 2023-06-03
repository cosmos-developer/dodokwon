#!/bin/bash

WALLET=$1
shift 1
KEYRING_BACKEND="test"

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
        --cw20-base)
            CW20_BASE_PATH=$2
            shift
            ;;
        --crowd-sale)
            CROWD_SALE_PATH=$2
            shift
            ;;
        --foundation)
            FOUNDATION_PATH=$2
            shift
            ;;
        *)
            ;;
    esac

    shift
done

NODE="--node $RPC"
TXFLAG="$NODE --chain-id $CHAIN_ID --gas-prices 100uluna --gas 1000000000 --keyring-backend $KEYRING_BACKEND "

MAX_ATTEMPTS=2
SLEEP_TIME=5

mkdir -p store

if [ -n "$CW20_BASE_PATH" ]; then
    echo -e "\nStoring code cw20-base..."
    TX=$(terrad tx wasm store $CW20_BASE_PATH --from $WALLET $TXFLAG --output json -y)
    echo $TX
    TX=$(echo $TX | jq -r '.txhash')
    echo "Store cw20 base contract tx hash: $TX"

    attempts=0
    success=false

    while [ $attempts -lt $MAX_ATTEMPTS ] && [ "$success" = false ]; do
        sleep $SLEEP_TIME
        
        QUERY=$(terrad query tx $TX $NODE --output json)
        CODE_ID=$(echo $QUERY | jq -r '.logs[0].events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_id") | .value')
        CODE_CHECKSUM=$(echo $QUERY | jq -r '.logs[0].events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_checksum") | .value')
        STORE_DATA="{\"tx\":\"$TX\",\"code_id\":$CODE_ID,\"code_checksum\":\"$CODE_CHECKSUM\"}"
       
        if [ -n "$CODE_ID" ]; then
            echo $STORE_DATA > store/cw20-base-store-data.json
            echo "Store info cw20 base contract: ./store/cw20-base-store-data.json"
            success=true
        else
            attempts=$((attempts + 1))
        fi
    done
    if [ "$success" = false ]; then
        echo "Exceeded maximum attempts. Unable to retrieve tx hash cw20-base contract."
        exit 1
    fi
fi

if [ -n "$CROWD_SALE_PATH" ]; then
    echo -e "\nStoring code crowd-sale..."
    TX=$(terrad tx wasm store $CROWD_SALE_PATH --from $WALLET $TXFLAG --output json -y | jq -r '.txhash')
    echo "Store crowd sale contract tx hash: $TX"

    attempts=0
    success=false

    while [ $attempts -lt $MAX_ATTEMPTS ] && [ "$success" = false ]; do
        sleep $SLEEP_TIME

        QUERY=$(terrad query tx $TX $NODE --output json)
        CODE_ID=$(echo $QUERY | jq -r '.logs[0].events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_id") | .value')
        CODE_CHECKSUM=$(echo $QUERY | jq -r '.logs[0].events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_checksum") | .value')
        STORE_DATA="{\"tx\":\"$TX\",\"code_id\":$CODE_ID,\"code_checksum\":\"$CODE_CHECKSUM\"}"
       
        if [ -n "$CODE_ID" ]; then
            echo $STORE_DATA > store/crowd-sale-store-data.json
            echo "Store info cw20 base contract: ./store/crowd-sale-store-data.json"
            success=true
        else
            attempts=$((attempts + 1))
        fi
    done
    if [ "$success" = false ]; then
        echo "Exceeded maximum attempts. Unable to retrieve tx hash crowd-sale contract."
        exit 1
    fi
fi


if [ -n "$FOUNDATION_PATH" ]; then
    echo -e "\nStoring code foundation..."
    TX=$(terrad tx wasm store $FOUNDATION_PATH --from $WALLET $TXFLAG --output json -y | jq -r '.txhash')
    echo "Store foundation contract tx hash: $TX"

    attempts=0
    success=false

    while [ $attempts -lt $MAX_ATTEMPTS ] && [ "$success" = false ]; do
        sleep $SLEEP_TIME
        
        QUERY=$(terrad query tx $TX $NODE --output json)
        CODE_ID=$(echo $QUERY | jq -r '.logs[0].events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_id") | .value')
        CODE_CHECKSUM=$(echo $QUERY | jq -r '.logs[0].events[] | select(.type == "store_code") | .attributes[] | select(.key == "code_checksum") | .value')
        STORE_DATA="{\"tx\":\"$TX\",\"code_id\":$CODE_ID,\"code_checksum\":\"$CODE_CHECKSUM\"}"
       
        if [ -n "$CODE_ID" ]; then
            echo $STORE_DATA > store/foundation-store-data.json
            echo "Store info cw20 base contract: ./store/foundation-store-data.json"
            success=true
        else
            attempts=$((attempts + 1))
        fi
    done
    if [ "$success" = false ]; then
        echo "Exceeded maximum attempts. Unable to retrieve tx hash foundation contract."
        exit 1
    fi
fi