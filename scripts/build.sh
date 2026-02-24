#!/bin/bash
# MEV Protocol - Full Build Script

set -e

echo "=================================="
echo "MEV Protocol - Build Script"
echo "=================================="

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

# Build C Hot Path
echo -e "\n${GREEN}[1/4] Building C Hot Path...${NC}"
cd fast
make clean
make
cd ..

# Build Rust Core
echo -e "\n${GREEN}[2/4] Building Rust Core...${NC}"
cd core
cargo build --release
cd ..

# Build Go Network
echo -e "\n${GREEN}[3/4] Building Go Network...${NC}"
cd network
go build -o ../bin/mev-node ./cmd/mev-node
cd ..

# Build Solidity Contracts
echo -e "\n${GREEN}[4/4] Building Solidity Contracts...${NC}"
cd contracts
forge build
cd ..

echo -e "\n${GREEN}=================================="
echo "Build Complete!"
echo "==================================${NC}"

echo -e "\nBinaries:"
echo "  - core/target/release/mev-engine"
echo "  - bin/mev-node"
echo "  - contracts/out/"
echo "  - fast/lib/libmev_fast.a"
