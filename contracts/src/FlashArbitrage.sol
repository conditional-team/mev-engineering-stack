// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import {IBalancerVault, IFlashLoanRecipient} from "./interfaces/IBalancerVault.sol";
import {IUniswapV3Pool} from "./interfaces/IUniswapV3.sol";
import {IUniswapV2Router} from "./interfaces/IUniswapV2.sol";
import {IERC20} from "./interfaces/IERC20.sol";
import {YulUtils} from "./libraries/YulUtils.sol";

/// @title FlashArbitrage
/// @author MEV Protocol Team
/// @notice High-performance flash loan arbitrage executor
/// @dev Optimized with inline Yul for gas efficiency
contract FlashArbitrage is IFlashLoanRecipient {
    /*//////////////////////////////////////////////////////////////
                                 ERRORS
    //////////////////////////////////////////////////////////////*/

    error Unauthorized();
    error InsufficientProfit();
    error InvalidCallback();
    error SwapFailed();
    error TransferFailed();

    /*//////////////////////////////////////////////////////////////
                                 EVENTS
    //////////////////////////////////////////////////////////////*/

    event ArbitrageExecuted(
        address indexed token,
        uint256 amountIn,
        uint256 profit,
        bytes32 indexed pathHash
    );

    event ProfitWithdrawn(address indexed token, uint256 amount);

    /*//////////////////////////////////////////////////////////////
                               CONSTANTS
    //////////////////////////////////////////////////////////////*/

    /// @dev Balancer Vault address (same on all chains)
    IBalancerVault public constant BALANCER_VAULT = 
        IBalancerVault(0xBA12222222228d8Ba445958a75a0704d566BF2C8);

    /// @dev Minimum profit threshold in basis points (0.1%)
    uint256 public constant MIN_PROFIT_BPS = 10;

    /// @dev Basis points denominator
    uint256 private constant BPS = 10000;

    /*//////////////////////////////////////////////////////////////
                                STORAGE
    //////////////////////////////////////////////////////////////*/

    /// @notice Contract owner
    address public immutable owner;

    /// @notice Whitelisted executors
    mapping(address => bool) public executors;

    /// @notice Pause state
    bool public paused;

    /// @notice Nonce for replay protection
    uint256 public nonce;

    /*//////////////////////////////////////////////////////////////
                              CONSTRUCTOR
    //////////////////////////////////////////////////////////////*/

    constructor() {
        owner = msg.sender;
        executors[msg.sender] = true;
    }

    /*//////////////////////////////////////////////////////////////
                               MODIFIERS
    //////////////////////////////////////////////////////////////*/

    modifier onlyOwner() {
        if (msg.sender != owner) revert Unauthorized();
        _;
    }

    modifier onlyExecutor() {
        if (!executors[msg.sender]) revert Unauthorized();
        _;
    }

    modifier notPaused() {
        require(!paused, "Paused");
        _;
    }

    /*//////////////////////////////////////////////////////////////
                            FLASH LOAN LOGIC
    //////////////////////////////////////////////////////////////*/

    /// @notice Execute flash loan arbitrage
    /// @param token Token to borrow
    /// @param amount Amount to borrow
    /// @param swapData Encoded swap path data
    function executeArbitrage(
        address token,
        uint256 amount,
        bytes calldata swapData
    ) external onlyExecutor notPaused {
        // Prepare flash loan
        address[] memory tokens = new address[](1);
        tokens[0] = token;
        
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = amount;

        // Encode callback data
        bytes memory userData = abi.encode(msg.sender, swapData);

        // Execute flash loan (callback will be triggered)
        BALANCER_VAULT.flashLoan(
            IFlashLoanRecipient(address(this)),
            tokens,
            amounts,
            userData
        );
    }

    /// @notice Balancer flash loan callback
    /// @dev Called by Balancer Vault after flash loan is issued
    function receiveFlashLoan(
        address[] memory tokens,
        uint256[] memory amounts,
        uint256[] memory feeAmounts,
        bytes memory userData
    ) external override {
        // Verify callback is from Balancer
        if (msg.sender != address(BALANCER_VAULT)) revert InvalidCallback();

        // Decode user data
        (address executor, bytes memory swapData) = abi.decode(userData, (address, bytes));
        
        // Verify executor
        if (!executors[executor]) revert Unauthorized();

        address token = tokens[0];
        uint256 amount = amounts[0];
        uint256 fee = feeAmounts[0];

        // Get balance before swaps
        uint256 balanceBefore = _balanceOf(token);

        // Execute swap sequence
        _executeSwaps(token, amount, swapData);

        // Calculate profit
        uint256 balanceAfter = _balanceOf(token);
        uint256 amountOwed = amount + fee;
        
        if (balanceAfter < amountOwed) revert InsufficientProfit();
        
        uint256 profit = balanceAfter - amountOwed;
        
        // Verify minimum profit
        uint256 minProfit = (amount * MIN_PROFIT_BPS) / BPS;
        if (profit < minProfit) revert InsufficientProfit();

        // Repay flash loan
        _safeTransfer(token, address(BALANCER_VAULT), amountOwed);

        // Emit event
        emit ArbitrageExecuted(token, amount, profit, keccak256(swapData));
    }

    /*//////////////////////////////////////////////////////////////
                             SWAP EXECUTION
    //////////////////////////////////////////////////////////////*/

    /// @notice Execute swap sequence
    /// @param token Starting token
    /// @param amount Starting amount
    /// @param swapData Encoded swap instructions
    function _executeSwaps(
        address token,
        uint256 amount,
        bytes memory swapData
    ) internal {
        // Decode swap count
        uint8 swapCount;
        assembly {
            swapCount := mload(add(swapData, 32))
        }

        uint256 offset = 33; // 32 bytes length + 1 byte count
        uint256 currentAmount = amount;
        address currentToken = token;

        for (uint8 i = 0; i < swapCount; i++) {
            // Decode swap type (1 = UniV2, 2 = UniV3, 3 = Curve)
            uint8 swapType;
            address target;
            bytes memory params;

            assembly {
                swapType := mload(add(swapData, add(offset, 1)))
                target := mload(add(swapData, add(offset, 21)))
            }
            
            offset += 21;
            
            // Read params length and data
            uint256 paramsLength;
            assembly {
                paramsLength := mload(add(swapData, add(offset, 32)))
            }
            offset += 32;
            
            params = new bytes(paramsLength);
            for (uint256 j = 0; j < paramsLength; j++) {
                params[j] = swapData[offset + j];
            }
            offset += paramsLength;

            // Execute swap based on type
            if (swapType == 1) {
                currentAmount = _swapUniV2(target, currentToken, currentAmount, params);
            } else if (swapType == 2) {
                currentAmount = _swapUniV3(target, currentToken, currentAmount, params);
            }
            
            // Update current token for next swap
            (currentToken) = abi.decode(params, (address));
        }
    }

    /// @notice Execute Uniswap V2 style swap
    function _swapUniV2(
        address router,
        address tokenIn,
        uint256 amountIn,
        bytes memory params
    ) internal returns (uint256 amountOut) {
        (address tokenOut, uint256 minOut) = abi.decode(params, (address, uint256));
        
        // Approve router
        _safeApprove(tokenIn, router, amountIn);
        
        // Build path
        address[] memory path = new address[](2);
        path[0] = tokenIn;
        path[1] = tokenOut;
        
        // Execute swap
        uint256[] memory amounts = IUniswapV2Router(router).swapExactTokensForTokens(
            amountIn,
            minOut,
            path,
            address(this),
            block.timestamp
        );
        
        amountOut = amounts[amounts.length - 1];
    }

    /// @notice Execute Uniswap V3 style swap
    function _swapUniV3(
        address pool,
        address tokenIn,
        uint256 amountIn,
        bytes memory params
    ) internal returns (uint256 amountOut) {
        (address tokenOut, uint24 fee, uint160 sqrtPriceLimitX96) = 
            abi.decode(params, (address, uint24, uint160));
        
        bool zeroForOne = tokenIn < tokenOut;
        
        // Execute swap on pool
        (int256 amount0, int256 amount1) = IUniswapV3Pool(pool).swap(
            address(this),
            zeroForOne,
            int256(amountIn),
            sqrtPriceLimitX96 == 0 
                ? (zeroForOne ? 4295128740 : 1461446703485210103287273052203988822378723970341)
                : sqrtPriceLimitX96,
            abi.encode(tokenIn, tokenOut)
        );
        
        amountOut = uint256(zeroForOne ? -amount1 : -amount0);
    }

    /*//////////////////////////////////////////////////////////////
                         YUL OPTIMIZED HELPERS
    //////////////////////////////////////////////////////////////*/

    /// @notice Get token balance using inline assembly
    function _balanceOf(address token) internal view returns (uint256 bal) {
        assembly {
            // Store selector for balanceOf(address)
            mstore(0x00, 0x70a0823100000000000000000000000000000000000000000000000000000000)
            mstore(0x04, address())
            
            // Call token
            let success := staticcall(gas(), token, 0x00, 0x24, 0x00, 0x20)
            
            if iszero(success) {
                revert(0, 0)
            }
            
            bal := mload(0x00)
        }
    }

    /// @notice Safe transfer using inline assembly
    function _safeTransfer(address token, address to, uint256 amount) internal {
        assembly {
            // Store selector for transfer(address,uint256)
            mstore(0x00, 0xa9059cbb00000000000000000000000000000000000000000000000000000000)
            mstore(0x04, to)
            mstore(0x24, amount)
            
            let success := call(gas(), token, 0, 0x00, 0x44, 0x00, 0x20)
            
            // Check return value
            if iszero(success) {
                revert(0, 0)
            }
            
            // Some tokens don't return a value
            if gt(returndatasize(), 0) {
                if iszero(mload(0x00)) {
                    revert(0, 0)
                }
            }
        }
    }

    /// @notice Safe approve using inline assembly
    function _safeApprove(address token, address spender, uint256 amount) internal {
        assembly {
            // Store selector for approve(address,uint256)
            mstore(0x00, 0x095ea7b300000000000000000000000000000000000000000000000000000000)
            mstore(0x04, spender)
            mstore(0x24, amount)
            
            let success := call(gas(), token, 0, 0x00, 0x44, 0x00, 0x20)
            
            if iszero(success) {
                revert(0, 0)
            }
        }
    }

    /*//////////////////////////////////////////////////////////////
                              ADMIN FUNCTIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Add or remove executor
    function setExecutor(address executor, bool status) external onlyOwner {
        executors[executor] = status;
    }

    /// @notice Pause/unpause contract
    function setPaused(bool _paused) external onlyOwner {
        paused = _paused;
    }

    /// @notice Withdraw profits
    function withdraw(address token, uint256 amount) external onlyOwner {
        _safeTransfer(token, owner, amount);
        emit ProfitWithdrawn(token, amount);
    }

    /// @notice Withdraw all ETH
    function withdrawETH() external onlyOwner {
        (bool success,) = owner.call{value: address(this).balance}("");
        require(success, "ETH transfer failed");
    }

    /// @notice Emergency token rescue
    function rescueToken(address token) external onlyOwner {
        uint256 balance = _balanceOf(token);
        _safeTransfer(token, owner, balance);
    }

    /*//////////////////////////////////////////////////////////////
                              UNISWAP V3 CALLBACK
    //////////////////////////////////////////////////////////////*/

    /// @notice Uniswap V3 swap callback
    function uniswapV3SwapCallback(
        int256 amount0Delta,
        int256 amount1Delta,
        bytes calldata data
    ) external {
        (address tokenIn, address tokenOut) = abi.decode(data, (address, address));
        
        // Pay the required input amount
        uint256 amountToPay = amount0Delta > 0 ? uint256(amount0Delta) : uint256(amount1Delta);
        address tokenToPay = amount0Delta > 0 ? tokenIn : tokenOut;
        
        _safeTransfer(tokenToPay, msg.sender, amountToPay);
    }

    /*//////////////////////////////////////////////////////////////
                               RECEIVE ETH
    //////////////////////////////////////////////////////////////*/

    receive() external payable {}
}
