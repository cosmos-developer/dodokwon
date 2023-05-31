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
        --crowd_sale_address)
            CROWD_SALE_ADDRESS=$2
            ;;
        *)
            ;;
    esac

    shift
done

NODE="--node $RPC"
TXFLAG="$NODE --chain-id $CHAIN_ID --gas-prices 2500000uluna --gas auto --gas-adjustment 1.3"
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

    echo -e "\n\nInstantiating contract..."
    INITIAL_STATE="{\"name\" : \"MyCoin\", \"symbol\": \"MCO\", \"decimals\": 6, \"initial_balances\": [{\"address\": $WALLET_ADDRESS, \"amount\": \"3000\"}], \"mint\": {\"minter\": $WALLET_ADDRESS, \"cap\": \"100000}}"
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

# 3. Update 
if [ "$ACTION" = "execute" ]; then
    if [ -z "$CONTRACT_ADDR" ]; then
        echo "Expected --contract-address"
        exit 1
    fi

    UPDATE_MINTER_ARGS="{\"update_minter\":{\"new_minter\": \"$CROWD_SALE_ADDRESS\"}}" 
    echo "Update Minter:"
    terrad tx wasm execute $CONTRACT_ADDR $UPDATE_MINTER_ARGS --from $WALLET $TXFLAG -y --output json
fi

# 4. Query
if [ "$ACTION" = "all" ] || [ "$ACTION" = "query" ]; then
    if [ -z "$CONTRACT_ADDR" ]; then
        echo "Expected --contract-address"
        exit 1
    fi

    if [ -z "$CROWD_SALE_ADDRESS" ]; then
        echo "Expected --crowd-sale-address"
        exit 1
    fi

    QUERY_TOKEN_INFO_ARGS="{\"token_info\":{}}" 
    echo "Token info:"
    terrad query wasm contract-state smart $CONTRACT_ADDR $QUERY_TOKEN_INFO_ARGS $NODE

    QUERY_BALANCE_ARGS="{\"balance\":{\"address\":\"$WALLET_ADDRESS\"}}" 
    echo "Foundation balance:"
    terrad query wasm contract-state smart $CONTRACT_ADDR $QUERY_BALANCE_ARGS $NODE
fi

# # 3. Execute transfer
# CONTRACT_ADDR=terra1gt034gdyre2nmkuy4x6zyyhw042uwcmkwczmp337wm9qkxhmrvmq8mq48s
# RECEIVER_ADDR=terra16ejk95unt3hg8eja90xh0tqgccgvcmwdxw2ed8
# echo -e "\n\nExecuting transfer to $RECEIVER_ADDR..."
# TRANSFER_ARGS="{\"transfer\":{\"recipient\":\"$RECEIVER_ADDR\",\"amount\":\"500\"}}"

# TRANSFER=$(terrad tx wasm execute $CONTRACT_ADDR $TRANSFER_ARGS --from $WALLET $TXFLAG -y --output json)
# echo "Transfer $TRANSFER"
# TRANSFER_TX_HASH=$(echo $TRANSFER | jq -r .txhash)

# echo "Transfer tx hash: $TRANSFER_TX_HASH"

# BLOCKCHAIN_STATUS=$(terrad status --node http://85.214.56.241:26657)
# blockchain_block_timestamp=$(echo $BLOCKCHAIN_STATUS | jq -r '.SyncInfo.latest_block_time' | xargs -I {} date -d {} +%s)
# blockchain_block_height=$(echo $BLOCKCHAIN_STATUS | jq -r '.SyncInfo.latest_block_height')
# sleep 5

# TRANSFER_TX_QUERY=$(terrad query tx $TRANSFER_TX_HASH --node http://85.214.56.241:26657 --output json)
# echo "Transfer Query: $TRANSFER_TX_QUERY"

# block_timestamp=$(echo $TRANSFER_TX_QUERY | jq -r '.logs[0].events[].attributes[] | select(.key == "block_timestamp").value')
# block_height=$(echo $TRANSFER_TX_QUERY | jq -r '.logs[0].events[].attributes[] | select(.key == "block_height").value')
# exchange_rate=$(echo $TRANSFER_TX_QUERY | jq -r '.logs[0].events[].attributes[] | select(.key == "exchange_rate").value')

# echo "Contract Block timestamp: $block_timestamp"
# echo "Contract Block height: $block_height"
# echo "Contract uluna/uusdt: $exchange_rate"

# echo -e "\n\nBlockchain Block timestamp: $blockchain_block_timestamp"
# echo "Blockchain Block height: $blockchain_block_height"

# # 4. Query state
# QUERY_BALANCE_ARGS="{\"balance\":{\"address\":\"$RECEIVER_ADDR\"}}" 
# RECEIVER_BALANCE=$(terrad query wasm contract-state smart $CONTRACT_ADDR $QUERY_BALANCE_ARGS $NODE --output json | jq -r .data.balance)
# echo -e "\n\nBalance of address $RECEIVER_ADDR: $RECEIVER_BALANCE"