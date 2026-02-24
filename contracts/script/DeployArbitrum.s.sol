// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "../src/FlashArbitrage.sol";

/**
 * @title DeployArbitrum
 * @notice Deploy script specifically for Arbitrum (mainnet and Sepolia testnet)
 * 
 * Usage:
 *   # Testnet (Sepolia)
 *   forge script script/DeployArbitrum.s.sol:DeployArbitrumSepolia --rpc-url $ARBITRUM_SEPOLIA_RPC --broadcast --verify
 *   
 *   # Mainnet
 *   forge script script/DeployArbitrum.s.sol:DeployArbitrumMainnet --rpc-url $ARBITRUM_RPC --broadcast --verify
 */

contract DeployArbitrumSepolia is Script {
    // Arbitrum Sepolia addresses
    address constant BALANCER_VAULT = address(0); // No Balancer on Sepolia - use mock
    
    function run() external {
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(deployerKey);
        
        console.log("Deploying to Arbitrum Sepolia");
        console.log("Deployer:", deployer);
        
        vm.startBroadcast(deployerKey);
        
        // For testnet, we'll need to deploy a mock Balancer vault first
        // or use a different flash loan provider
        
        // Deploy mock for testing
        MockBalancerVault mockVault = new MockBalancerVault();
        console.log("Mock Balancer Vault:", address(mockVault));
        
        // Deploy main contract (uses hardcoded Balancer address)
        FlashArbitrage arb = new FlashArbitrage();
        console.log("FlashArbitrage:", address(arb));
        
        vm.stopBroadcast();
        
        // Save addresses
        string memory output = string(abi.encodePacked(
            '{"network":"arbitrum-sepolia","balancer":"',
            vm.toString(address(mockVault)),
            '","flashArbitrage":"',
            vm.toString(address(arb)),
            '"}'
        ));
        
        vm.writeFile("deployments/arbitrum-sepolia.json", output);
    }
}

contract DeployArbitrumMainnet is Script {
    // Arbitrum Mainnet addresses
    address constant BALANCER_VAULT = 0xBA12222222228d8Ba445958a75a0704d566BF2C8;
    
    function run() external {
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(deployerKey);
        
        console.log("Deploying to Arbitrum Mainnet");
        console.log("Deployer:", deployer);
        console.log("Balance:", deployer.balance);
        
        vm.startBroadcast(deployerKey);
        
        // Deploy main contract (Balancer Vault is hardcoded in contract)
        FlashArbitrage arb = new FlashArbitrage();
        console.log("FlashArbitrage deployed:", address(arb));
        
        vm.stopBroadcast();
        
        // Save addresses
        string memory output = string(abi.encodePacked(
            '{"network":"arbitrum","balancer":"',
            vm.toString(BALANCER_VAULT),
            '","flashArbitrage":"',
            vm.toString(address(arb)),
            '"}'
        ));
        
        vm.writeFile("deployments/arbitrum.json", output);
        
        console.log("\n=== DEPLOYMENT COMPLETE ===");
        console.log("FlashArbitrage:", address(arb));
        console.log("Balancer Vault:", BALANCER_VAULT);
        console.log("\nNext steps:");
        console.log("1. Verify contract on Arbiscan");
        console.log("2. Update config/.env with contract address");
        console.log("3. Fund contract with ETH for gas");
    }
}

/// @notice Mock Balancer Vault for testnet
contract MockBalancerVault {
    event FlashLoan(
        address indexed recipient,
        address[] tokens,
        uint256[] amounts,
        uint256[] feeAmounts
    );
    
    function flashLoan(
        address recipient,
        address[] calldata tokens,
        uint256[] calldata amounts,
        bytes calldata userData
    ) external {
        uint256[] memory feeAmounts = new uint256[](tokens.length);
        
        // Transfer tokens to recipient (in real scenario)
        // For testing, just emit event and callback
        
        emit FlashLoan(recipient, tokens, amounts, feeAmounts);
        
        // Callback
        (bool success,) = recipient.call(
            abi.encodeWithSignature(
                "receiveFlashLoan(address[],uint256[],uint256[],bytes)",
                tokens,
                amounts,
                feeAmounts,
                userData
            )
        );
        require(success, "Callback failed");
    }
}
