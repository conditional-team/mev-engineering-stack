#!/bin/bash
# MEV Protocol - Contract Deployment Script

set -e

echo "=================================="
echo "MEV Protocol - Deploy Contracts"
echo "=================================="

# Load environment
source config/.env

# Check required variables
if [ -z "$PRIVATE_KEY" ]; then
    echo "Error: PRIVATE_KEY not set"
    exit 1
fi

# Parse arguments
CHAIN=${1:-"arbitrum"}

case $CHAIN in
    "ethereum"|"mainnet")
        RPC_URL=$ETH_RPC_URL
        CHAIN_ID=1
        ;;
    "arbitrum"|"arb")
        RPC_URL=$ARBITRUM_RPC_URL
        CHAIN_ID=42161
        ;;
    "base")
        RPC_URL=$BASE_RPC_URL
        CHAIN_ID=8453
        ;;
    "optimism"|"op")
        RPC_URL=$OPTIMISM_RPC_URL
        CHAIN_ID=10
        ;;
    *)
        echo "Unknown chain: $CHAIN"
        echo "Usage: ./deploy.sh [ethereum|arbitrum|base|optimism]"
        exit 1
        ;;
esac

echo "Deploying to $CHAIN (Chain ID: $CHAIN_ID)"
echo "RPC: $RPC_URL"

# Deploy
cd contracts

forge script script/Deploy.s.sol:DeployScript \
    --rpc-url $RPC_URL \
    --private-key $PRIVATE_KEY \
    --broadcast \
    --verify \
    -vvvv

echo ""
echo "Deployment complete!"
echo "Update your .env with the deployed addresses"
