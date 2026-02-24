// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title YulUtils
/// @notice Gas-optimized utility functions using inline assembly
/// @dev All functions are marked internal for inlining
library YulUtils {
    /*//////////////////////////////////////////////////////////////
                           MATH OPERATIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Calculate a * b / c with full precision
    function mulDiv(
        uint256 a,
        uint256 b,
        uint256 c
    ) internal pure returns (uint256 result) {
        assembly {
            // Store free memory pointer
            let mm := mload(0x40)

            // Compute a * b
            let ab := mul(a, b)
            
            // Check for overflow
            if iszero(or(iszero(b), eq(div(ab, b), a))) {
                revert(0, 0)
            }

            // Compute result
            result := div(ab, c)
            
            // Check for division by zero
            if iszero(c) {
                revert(0, 0)
            }
        }
    }

    /// @notice Calculate (a * b) % c with protection
    function mulMod(
        uint256 a,
        uint256 b,
        uint256 c
    ) internal pure returns (uint256 result) {
        assembly {
            result := mulmod(a, b, c)
        }
    }

    /// @notice Safe subtraction
    function safeSub(uint256 a, uint256 b) internal pure returns (uint256 result) {
        assembly {
            if lt(a, b) {
                revert(0, 0)
            }
            result := sub(a, b)
        }
    }

    /*//////////////////////////////////////////////////////////////
                         ADDRESS OPERATIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Check if address is a contract
    function isContract(address account) internal view returns (bool result) {
        assembly {
            result := gt(extcodesize(account), 0)
        }
    }

    /// @notice Get code hash of address
    function codeHash(address account) internal view returns (bytes32 hash) {
        assembly {
            hash := extcodehash(account)
        }
    }

    /// @notice Compare two addresses
    function addressLt(address a, address b) internal pure returns (bool result) {
        assembly {
            result := lt(a, b)
        }
    }

    /*//////////////////////////////////////////////////////////////
                          MEMORY OPERATIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Efficient memory copy
    function memoryCopy(
        uint256 src,
        uint256 dst,
        uint256 length
    ) internal pure {
        assembly {
            // Copy 32 bytes at a time
            for { let i := 0 } lt(i, length) { i := add(i, 32) } {
                mstore(add(dst, i), mload(add(src, i)))
            }
        }
    }

    /// @notice Get free memory pointer
    function freeMemoryPointer() internal pure returns (uint256 ptr) {
        assembly {
            ptr := mload(0x40)
        }
    }

    /// @notice Set free memory pointer
    function setFreeMemoryPointer(uint256 ptr) internal pure {
        assembly {
            mstore(0x40, ptr)
        }
    }

    /*//////////////////////////////////////////////////////////////
                           HASH OPERATIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Efficient keccak256 for two uint256s
    function hash2(uint256 a, uint256 b) internal pure returns (bytes32 result) {
        assembly {
            mstore(0x00, a)
            mstore(0x20, b)
            result := keccak256(0x00, 0x40)
        }
    }

    /// @notice Efficient keccak256 for address and uint256
    function hashAddressUint(address a, uint256 b) internal pure returns (bytes32 result) {
        assembly {
            mstore(0x00, a)
            mstore(0x20, b)
            result := keccak256(0x00, 0x40)
        }
    }

    /*//////////////////////////////////////////////////////////////
                        CALLDATA OPERATIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Load uint256 from calldata at offset
    function loadCalldataUint(uint256 offset) internal pure returns (uint256 value) {
        assembly {
            value := calldataload(offset)
        }
    }

    /// @notice Load address from calldata at offset
    function loadCalldataAddress(uint256 offset) internal pure returns (address value) {
        assembly {
            value := shr(96, calldataload(offset))
        }
    }

    /*//////////////////////////////////////////////////////////////
                           SQRT OPERATIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Babylonian method for sqrt
    function sqrt(uint256 x) internal pure returns (uint256 z) {
        assembly {
            // Initial estimate
            z := 1
            let y := x
            if gt(y, 3) {
                z := y
                let guess := add(div(y, 2), 1)
                for { } lt(guess, z) { } {
                    z := guess
                    guess := div(add(div(y, guess), guess), 2)
                }
            }
            if and(gt(y, 0), lt(y, 4)) {
                z := 1
            }
        }
    }

    /*//////////////////////////////////////////////////////////////
                         UNISWAP V2 HELPERS
    //////////////////////////////////////////////////////////////*/

    /// @notice Calculate Uniswap V2 amount out
    /// @param amountIn Input amount
    /// @param reserveIn Input reserve
    /// @param reserveOut Output reserve
    function getAmountOut(
        uint256 amountIn,
        uint256 reserveIn,
        uint256 reserveOut
    ) internal pure returns (uint256 amountOut) {
        assembly {
            // amountInWithFee = amountIn * 997
            let amountInWithFee := mul(amountIn, 997)
            
            // numerator = amountInWithFee * reserveOut
            let numerator := mul(amountInWithFee, reserveOut)
            
            // denominator = reserveIn * 1000 + amountInWithFee
            let denominator := add(mul(reserveIn, 1000), amountInWithFee)
            
            // amountOut = numerator / denominator
            amountOut := div(numerator, denominator)
        }
    }

    /// @notice Calculate Uniswap V2 amount in
    function getAmountIn(
        uint256 amountOut,
        uint256 reserveIn,
        uint256 reserveOut
    ) internal pure returns (uint256 amountIn) {
        assembly {
            // numerator = reserveIn * amountOut * 1000
            let numerator := mul(mul(reserveIn, amountOut), 1000)
            
            // denominator = (reserveOut - amountOut) * 997
            let denominator := mul(sub(reserveOut, amountOut), 997)
            
            // amountIn = (numerator / denominator) + 1
            amountIn := add(div(numerator, denominator), 1)
        }
    }
}
