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
        --action)
            ACTION=$2
            shift
            ;;
        --uluna)
            ULUNA=$2
            shift
            ;;
        --voter)
            VOTER_WALLET=$2
            VOTER_ADDRESS=$(terrad keys list --output json | jq -r "[ .[] | select( .name == \"$VOTER_WALLET\") ][0].address")
            shift
            ;;
        *)
            ;;
    esac

    shift
done

NODE="--node $RPC"
TXFLAG="$NODE --chain-id $CHAIN_ID --gas-prices 250000000uluna --gas auto --gas-adjustment 1.3"
WALLET_ADDRESS=$(terrad keys list --output json | jq -r "[ .[] | select( .name == \"$WALLET\") ][0].address")

MAX_ATTEMPTS=2
SLEEP_TIME=5

CW20_BASE_ADDRESS=$(cat ./contract-addresses/cw20-base-address.txt)
FOUNDATION_ADDRESS=$(cat ./contract-addresses/foundation-address.txt)
CROWD_SALE_ADDRESS=$(cat ./contract-addresses/crowd-sale-address.txt)

# Buyer call mint in crowd sale contract
if [ "$ACTION" = "mint" ]; then
    if [ -z "$ULUNA" ]; then
        echo -e "Expect --uluna <amount>"
        exit 1
    fi
    echo -e "\n========== Buyer call mint in crowd sale contract =========="
    QUERY_WALLET_BALANCE_ARGS="{\"balance\":{\"address\":\"$WALLET_ADDRESS\"}}"
    INITIAL_BUYER_BALANCE=$(terrad query wasm contract-state smart $CW20_BASE_ADDRESS $QUERY_WALLET_BALANCE_ARGS $NODE --output json | jq -r .data.balance)
    echo -e "\nBefore mint Balance of Buyer: $INITIAL_BUYER_BALANCE"

    MINT_ARGS="{\"mint\":{}}"
    MINT=$(terrad tx wasm execute $CROWD_SALE_ADDRESS $MINT_ARGS --amount "$ULUNA"uluna --from $WALLET $TXFLAG  -y --output json)
    MINT_TX_HASH=$(echo $MINT | jq -r .txhash)
    echo -e "\nMint tx hash: $MINT_TX_HASH"

    sleep 5

    QUERY_WALLET_BALANCE_ARGS="{\"balance\":{\"address\":\"$WALLET_ADDRESS\"}}"
    AFTER_MINT_BUYER_BALANCE=$(terrad query wasm contract-state smart $CW20_BASE_ADDRESS $QUERY_WALLET_BALANCE_ARGS $NODE --output json | jq -r .data.balance)
    echo -e "\nAfter mint Balance of Buyer: $AFTER_MINT_BUYER_BALANCE"
fi

# Propose and Execute add new voter
if [ "$ACTION" = "add-voter" ]; then
    if [ -z "$VOTER_ADDRESS" ]; then
        echo -e "Expect --voter <voter-wallet-name>"
        exit 1
    fi

    echo -e "\n========== Add new voter: Propose + Execute =========="
    # Query voter list before add new voter
    QUERY_VOTER_LIST_ARGS="{\"list_voters\":{}}"
    QUERY_VOTER_LIST=$(terrad query wasm contract-state smart $FOUNDATION_ADDRESS $QUERY_VOTER_LIST_ARGS $NODE --output json | jq -r .data.voters)
    echo -e "\nVoter list before add new voter: $QUERY_VOTER_LIST"

    echo -e "\nProposing add voter $VOTER_ADDRESS..."
    PROPOSE_ADD_VOTER_ARGS="{\"propose\":{\"title\":\"Title - Add new voter\", \"description\":\"Description - Add new voter\",\"msgs\":[],\"proposal_type\":{\"add_voter\":{\"address\":\"$VOTER_ADDRESS\", \"vote_weight\":1}}}}"
    PROPOSE_ADD_VOTER=$(terrad tx wasm execute $FOUNDATION_ADDRESS "$PROPOSE_ADD_VOTER_ARGS" --from $WALLET $TXFLAG  -y --output json)
    PROPOSE_ADD_VOTER_TX_HASH=$(echo $PROPOSE_ADD_VOTER | jq -r .txhash)
    echo -e "Proposal add voter tx hash: $PROPOSE_ADD_VOTER_TX_HASH"
    
    sleep 5

    # Query proposal list
    QUERY_PROPOSAL_LIST_ARGS="{\"reverse_proposals\":{}}"
    QUERY_PROPOSAL_LIST=$(terrad query wasm contract-state smart $FOUNDATION_ADDRESS $QUERY_PROPOSAL_LIST_ARGS $NODE --output json)
    PROPOSAL_ID=$(echo $QUERY_PROPOSAL_LIST | jq -r ".data.proposals[0].id")
    echo "Proposal list: " 
    echo $QUERY_PROPOSAL_LIST | jq -r ".data.proposals"

    # Execute add voter
    echo -e "\nExecuting add voter..."
    EXECUTE_ADD_VOTER_ARGS="{\"execute\":{\"proposal_id\":$PROPOSAL_ID}}"
    EXECUTE_ADD_VOTER=$(terrad tx wasm execute $FOUNDATION_ADDRESS "$EXECUTE_ADD_VOTER_ARGS" --from $WALLET $TXFLAG  -y --output json)
    EXECUTE_ADD_VOTER_TX_HASH=$(echo $EXECUTE_ADD_VOTER | jq -r .txhash)
    echo -e "\nExecute add voter tx hash: $EXECUTE_ADD_VOTER_TX_HASH"

    sleep 5

    # Query voter list after add new voter
    QUERY_VOTER_LIST_ARGS="{\"list_voters\":{}}"
    QUERY_VOTER_LIST=$(terrad query wasm contract-state smart $FOUNDATION_ADDRESS $QUERY_VOTER_LIST_ARGS $NODE --output json | jq -r .data.voters)
    echo -e "\nVoter list after add new voter: $QUERY_VOTER_LIST"
fi

# Propose and Vote and Execute send cw20 token
if [ "$ACTION" = "send" ]; then 
    if [ -z "$VOTER_WALLET" ]; then
        echo "Expect --voter flag"
        exit 1
    fi

    # Propose send cw20
    echo -e "\n========== Propose send cw20: Propose + Vote + Execute =========="
    RECIPIENT_ADDRESS=terra1eegjquhhdfvlayawj9c4djnqy28956a3czszt4
    SEND_AMOUNT=100
    
    echo -e "\nProposing send $SEND_AMOUNT to address $RECIPIENT_ADDRESS..."
    PROPOSE_SEND_CW20_ARGS="{\"propose\":{\"title\":\"Send cw20 title\", \"description\":\"Send cw20 description\",\"msgs\":[],\"proposal_type\":{\"send\":{\"to\":\"$RECIPIENT_ADDRESS\", \"amount\":\"$SEND_AMOUNT\"}}}}"
    PROPOSE_SEND_CW20=$(terrad tx wasm execute $FOUNDATION_ADDRESS "$PROPOSE_SEND_CW20_ARGS" --from $WALLET $TXFLAG  -y --output json)
    PROPOSE_SEND_CW20_TX_HASH=$(echo $PROPOSE_SEND_CW20 | jq -r .txhash)
    echo -e "Propose send cw20 tx hash: $PROPOSE_SEND_CW20_TX_HASH"

    sleep 5

    # Query proposal list
    echo -e "\nLatest proposal:" 
    QUERY_PROPOSAL_LIST_ARGS="{\"reverse_proposals\":{}}"
    QUERY_PROPOSAL_LIST=$(terrad query wasm contract-state smart $FOUNDATION_ADDRESS $QUERY_PROPOSAL_LIST_ARGS $NODE --output json)
    PROPOSAL_ID=$(echo $QUERY_PROPOSAL_LIST | jq -r ".data.proposals[0].id")
    echo $QUERY_PROPOSAL_LIST | jq -r ".data.proposals[0]"

    # Vote send cw20
    echo -e "\nVoting send $SEND_AMOUNT to $RECIPIENT_ADDRESS..."
    VOTE_SEND_CW20_ARGS="{\"vote\":{\"proposal_id\":$PROPOSAL_ID, \"vote\":\"yes\"}}"
    VOTE_SEND_CW20=$(terrad tx wasm execute $FOUNDATION_ADDRESS "$VOTE_SEND_CW20_ARGS" --from $VOTER_WALLET $TXFLAG -y --output json)
    VOTE_SEND_CW20_TX_HASH=$(echo $VOTE_SEND_CW20 | jq -r .txhash)
    echo -e "Vote send cw20 tx hash: $VOTE_SEND_CW20_TX_HASH"

    sleep 5

    # Query proposal status
    QUERY_PROPOSAL_ARGS="{\"proposal\":{\"proposal_id\":$PROPOSAL_ID}}"
    QUERY_PROPOSAL=$(terrad query wasm contract-state smart $FOUNDATION_ADDRESS "$QUERY_PROPOSAL_ARGS" $NODE --output json | jq -r ".data")
    echo -e "\nProposal status:" $(echo $QUERY_PROPOSAL | jq -r '.status')

    # Execute send cw20
    echo -e "\nExecuting send $SEND_AMOUNT to $RECIPIENT_ADDRESS "
    EXECUTE_SEND_CW20_ARGS="{\"execute\":{\"proposal_id\":$PROPOSAL_ID}}"
    EXECUTE_SEND_CW20=$(terrad tx wasm execute $FOUNDATION_ADDRESS "$EXECUTE_SEND_CW20_ARGS" --from $WALLET $TXFLAG  -y --output json)
    EXECUTE_SEND_CW20_TX_HASH=$(echo $EXECUTE_SEND_CW20 | jq -r .txhash)
    echo -e "Execute send cw20 tx hash: $EXECUTE_SEND_CW20_TX_HASH"

    sleep 5

    # Query cw20 balance of recipient
    echo -e "\n========== Query cw20 balance of recipient =========="
    QUERY_CW20_BALANCE_ARGS="{\"balance\":{\"address\":\"$RECIPIENT_ADDRESS\"}}"
    QUERY_CW20_BALANCE=$(terrad query wasm contract-state smart $CW20_BASE_ADDRESS $QUERY_CW20_BALANCE_ARGS $NODE --output json | jq -r .data.balance)
    echo -e "\nCw20 balance of $RECIPIENT_ADDRESS: $QUERY_CW20_BALANCE"
fi

                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               

