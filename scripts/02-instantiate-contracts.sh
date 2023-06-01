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
        --only-contract)
            ONLY_CONTRACT=$2
            shift
            ;;
        *)
            ;;
    esac

    shift
done

NODE="--node $RPC"
TXFLAG="$NODE --chain-id $CHAIN_ID --gas-prices 25000000uluna --gas-adjustment 1.3"
WALLET_ADDRESS=$(terrad keys list --output json | jq -r "[ .[] | select( .name == \"$WALLET\") ][0].address")

MAX_ATTEMPTS=2
SLEEP_TIME=5

mkdir -p contract-addresses

if [ -z "$ONLY_CONTRACT" ] || [ "$ONLY_CONTRACT" = "cw20-base" ]; then
    CW20_CODE_ID=$(cat ./store/cw20-base-store-data.json | jq -r .code_id)

    echo -e "\n\nInstantiating cw20 base contract..."
    INITIAL_STATE="{\"name\" : \"MyCoin\", \"symbol\": \"MCO\", \"decimals\": 6, \"initial_balances\": [{\"address\": \"$WALLET_ADDRESS\", \"amount\": \"3000\"}], \"mint\": {\"minter\": \"$WALLET_ADDRESS\", \"cap\": \"100000\"}}"
    INSTANTIATE_TX=$(terrad tx wasm instantiate $CW20_CODE_ID "$INITIAL_STATE" --label "cw20-base" --from $WALLET $TXFLAG -y --no-admin --output json | jq -r .txhash)
    
    attempts=0
    success=false
    while [ $attempts -lt $MAX_ATTEMPTS ] && [ "$success" = false ]; do
        sleep $SLEEP_TIME
        CW20_BASE_ADDRESS=$(terrad query tx $INSTANTIATE_TX $NODE --output json | jq -r '.logs[0].events[] | select(.type == "instantiate") | .attributes[] | select (.key == "_contract_address") | .value')

        if [ -n "$CW20_BASE_ADDRESS" ]; then
            echo "Cw20 base contract address: $CW20_BASE_ADDRESS"
            echo $CW20_BASE_ADDRESS > ./contract-addresses/cw20-base-address.txt
            success=true
        else
            attempts=$((attempts + 1))
        fi
    done
    if [ "$success" = false ]; then
        echo "Exceeded maximum attempts. Unable to retrieve CW20_BASE_ADDRESS."
        exit 1
    fi
fi

if [ -z "$ONLY_CONTRACT" ] || [ "$ONLY_CONTRACT" = "crowd-sale" ]; then
    CW20_BASE_ADDRESS=$(cat ./contract-addresses/cw20-base-address.txt)
    if [ -z "$CW20_BASE_ADDRESS" ]; then
        echo "Instantiate cw20 base contract first."
        exit 1
    fi
    CROWD_SALE_CODE_ID=$(cat ./store/crowd-sale-store-data.json | jq -r .code_id)

    echo -e "\n\nInstantiating crowd sale contract..."
    INITIAL_STATE="{\"cw20_address\" : \"$CW20_BASE_ADDRESS\", \"mintable_period_days\": 30, \"udodokwan_per_uusd\": \"0.000000001\"}"
    INSTANTIATE_TX=$(terrad tx wasm instantiate $CROWD_SALE_CODE_ID "$INITIAL_STATE" --label "crowd-sale" --from $WALLET $TXFLAG -y --no-admin --output json | jq -r .txhash)

    attempts=0
    success=false
    while [ $attempts -lt $MAX_ATTEMPTS ] && [ "$success" = false ]; do
        sleep $SLEEP_TIME
        CONTRACT_ADDR=$(terrad query tx $INSTANTIATE_TX $NODE --output json | jq -r '.logs[0].events[] | select(.type == "instantiate") | .attributes[] | select (.key == "_contract_address") | .value')
        
        if [ -n "$CONTRACT_ADDR" ]; then
            echo "Crowd sale contract address: $CONTRACT_ADDR"
            echo $CONTRACT_ADDR > ./contract-addresses/crowd-sale-address.txt
            success=true
        else
            attempts=$((attempts + 1))
        fi
    done
    if [ "$success" = false ]; then
        echo "Exceeded maximum attempts. Unable to retrieve crowd sale address."
        exit 1
    fi
fi

if [ -z "$ONLY_CONTRACT" ] || [ "$ONLY_CONTRACT" = "foundation" ]; then
    CW20_BASE_ADDRESS=$(cat ./contract-addresses/cw20-base-address.txt)
    if [ -z "$CW20_BASE_ADDRESS" ]; then
        echo "Instantiate cw20 base contract first."
        exit 1
    fi

    FOUNDATION_CODE_ID=$(cat ./store/foundation-store-data.json | jq -r .code_id)

    echo -e "\n\nInstantiating foundation contract..."
    INITIAL_STATE="{\"cw20_address\" : \"$CW20_BASE_ADDRESS\", \"max_voting_period\": { \"height\": 300 }, \"threshold\": { \"absolute_percentage\": { \"percentage\": \"0.5\" } }, \"voters\": [{\"addr\": \"$WALLET_ADDRESS\", \"weight\": 1}]}"
    INSTANTIATE_TX=$(terrad tx wasm instantiate $FOUNDATION_CODE_ID "$INITIAL_STATE" --label "foundation" --from $WALLET $TXFLAG -y --no-admin --output json | jq -r .txhash)
        
    attempts=0
    success=false
    while [ $attempts -lt $MAX_ATTEMPTS ] && [ "$success" = false ]; do
        sleep $SLEEP_TIME
        CONTRACT_ADDR=$(terrad query tx $INSTANTIATE_TX $NODE --output json | jq -r '.logs[0].events[] | select(.type == "instantiate") | .attributes[] | select (.key == "_contract_address") | .value')

        if [ -n "$CONTRACT_ADDR" ]; then
            echo "Foundation contract address: $CONTRACT_ADDR"
            echo $CONTRACT_ADDR > ./contract-addresses/foundation-address.txt
            success=true
        else
            attempts=$((attempts + 1))
        fi
    done
    if [ "$success" = false ]; then
        echo "Exceeded maximum attempts. Unable to retrieve foundation address."
        exit 1
    fi
fi