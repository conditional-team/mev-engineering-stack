// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "../src/FlashArbitrage.sol";
import "../src/MultiDexRouter.sol";

contract DeployScript is Script {
    function run() public {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(deployerPrivateKey);
        
        console.log("Deployer:", deployer);
        console.log("Balance:", deployer.balance);
        
        vm.startBroadcast(deployerPrivateKey);

        // Deploy FlashArbitrage
        FlashArbitrage flashArb = new FlashArbitrage();
        console.log("FlashArbitrage deployed at:", address(flashArb));

        // Deploy MultiDexRouter
        MultiDexRouter router = new MultiDexRouter();
        console.log("MultiDexRouter deployed at:", address(router));

        // Add router as executor
        flashArb.setExecutor(address(router), true);
        console.log("Router added as executor");

        vm.stopBroadcast();

        // Log deployment summary
        console.log("\n=== DEPLOYMENT SUMMARY ===");
        console.log("Chain ID:", block.chainid);
        console.log("FlashArbitrage:", address(flashArb));
        console.log("MultiDexRouter:", address(router));
    }
}

contract DeployMultichain is Script {
    struct ChainConfig {
        string name;
        uint256 chainId;
        string rpcUrl;
    }

    function run() public {
        ChainConfig[] memory chains = new ChainConfig[](5);
        chains[0] = ChainConfig("Ethereum", 1, vm.envString("ETH_RPC_URL"));
        chains[1] = ChainConfig("Arbitrum", 42161, vm.envString("ARBITRUM_RPC_URL"));
        chains[2] = ChainConfig("Base", 8453, vm.envString("BASE_RPC_URL"));
        chains[3] = ChainConfig("Optimism", 10, vm.envString("OPTIMISM_RPC_URL"));
        chains[4] = ChainConfig("Polygon", 137, vm.envString("POLYGON_RPC_URL"));

        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");

        for (uint256 i = 0; i < chains.length; i++) {
            console.log("\n=== Deploying to", chains[i].name, "===");
            
            vm.createSelectFork(chains[i].rpcUrl);
            
            vm.startBroadcast(deployerPrivateKey);
            
            FlashArbitrage flashArb = new FlashArbitrage();
            MultiDexRouter router = new MultiDexRouter();
            flashArb.setExecutor(address(router), true);
            
            vm.stopBroadcast();
            
            console.log("FlashArbitrage:", address(flashArb));
            console.log("MultiDexRouter:", address(router));
        }
    }
}
