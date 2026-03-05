// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../src/MultiDexRouter.sol";

contract MultiDexRouterTest is Test {
    MultiDexRouter public router;
    address public attacker = address(0xBEEF);

    function setUp() public {
        router = new MultiDexRouter();
    }

    function test_ExecuteSwapPathRejectsMalformedInput() public {
        bytes memory malformed = hex"1234";

        vm.expectRevert(MultiDexRouter.InvalidInput.selector);
        router.executeSwapPath(malformed);
    }

    function test_UniswapV3CallbackSpoofReverts() public {
        vm.prank(attacker);
        vm.expectRevert(MultiDexRouter.InvalidCallback.selector);
        router.uniswapV3SwapCallback(1, 0, abi.encode(address(0x1), address(0x2)));
    }
}
