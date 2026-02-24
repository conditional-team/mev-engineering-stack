/**
 * MEV Protocol - C Hot Path
 * Calldata Parser
 */

#include "parser.h"
#include <string.h>

/* Known function selectors */
#define SEL_SWAP_EXACT_TOKENS_V2     0x38ed1739
#define SEL_SWAP_TOKENS_EXACT_V2     0x8803dbee
#define SEL_EXACT_INPUT_SINGLE_V3    0x414bf389
#define SEL_EXACT_INPUT_V3           0xc04b8d59
#define SEL_EXACT_OUTPUT_SINGLE_V3   0x5023b4df
#define SEL_EXACT_OUTPUT_V3          0xf28c0498
#define SEL_MULTICALL                0xac9650d8
#define SEL_EXECUTE                  0x3593564c

/**
 * Extract function selector from calldata
 */
uint32_t mev_parse_selector(const uint8_t *calldata, size_t calldata_len) {
    if (!calldata || calldata_len < 4) {
        return 0;
    }

    return ((uint32_t)calldata[0] << 24) |
           ((uint32_t)calldata[1] << 16) |
           ((uint32_t)calldata[2] << 8) |
           (uint32_t)calldata[3];
}

/**
 * Check if selector is a swap function
 */
int mev_is_swap_selector(uint32_t selector) {
    switch (selector) {
        case SEL_SWAP_EXACT_TOKENS_V2:
        case SEL_SWAP_TOKENS_EXACT_V2:
        case SEL_EXACT_INPUT_SINGLE_V3:
        case SEL_EXACT_INPUT_V3:
        case SEL_EXACT_OUTPUT_SINGLE_V3:
        case SEL_EXACT_OUTPUT_V3:
        case SEL_EXECUTE:
            return 1;
        default:
            return 0;
    }
}

/**
 * Decode uint256 from calldata at offset
 */
int mev_decode_uint256(const uint8_t *calldata, size_t calldata_len,
                       size_t offset, uint8_t *value) {
    if (!calldata || !value || offset + 32 > calldata_len) {
        return -1;
    }

    memcpy(value, calldata + offset, 32);
    return 0;
}

/**
 * Decode address from calldata at offset
 */
int mev_decode_address(const uint8_t *calldata, size_t calldata_len,
                       size_t offset, uint8_t *address) {
    if (!calldata || !address || offset + 32 > calldata_len) {
        return -1;
    }

    /* Address is right-aligned in 32 bytes */
    memcpy(address, calldata + offset + 12, 20);
    return 0;
}

/**
 * Parse UniswapV2 swap calldata
 */
int mev_parse_v2_swap(const uint8_t *calldata, size_t calldata_len,
                      mev_swap_info_t *info) {
    if (!calldata || !info || calldata_len < 164) {
        return -1;
    }

    uint32_t selector = mev_parse_selector(calldata, calldata_len);
    
    if (selector != SEL_SWAP_EXACT_TOKENS_V2 && 
        selector != SEL_SWAP_TOKENS_EXACT_V2) {
        return -1;
    }

    info->dex_type = DEX_UNISWAP_V2;

    /* Offset 4: amountIn (32 bytes) */
    mev_decode_uint256(calldata, calldata_len, 4, info->amount_in);

    /* Offset 36: amountOutMin (32 bytes) */
    mev_decode_uint256(calldata, calldata_len, 36, info->amount_out_min);

    /* Offset 68: path offset (points to dynamic array) */
    /* For simplicity, assume 2-token path at standard positions */
    
    /* Offset 132: first token */
    mev_decode_address(calldata, calldata_len, 132, info->token_in);
    
    /* Offset 164: second token (if exists) */
    if (calldata_len >= 196) {
        mev_decode_address(calldata, calldata_len, 164, info->token_out);
    }

    return 0;
}

/**
 * Parse UniswapV3 exactInputSingle calldata
 */
int mev_parse_v3_swap(const uint8_t *calldata, size_t calldata_len,
                      mev_swap_info_t *info) {
    if (!calldata || !info || calldata_len < 196) {
        return -1;
    }

    uint32_t selector = mev_parse_selector(calldata, calldata_len);
    
    if (selector != SEL_EXACT_INPUT_SINGLE_V3) {
        return -1;
    }

    info->dex_type = DEX_UNISWAP_V3;

    /* ExactInputSingleParams struct:
     * tokenIn (address) - offset 4
     * tokenOut (address) - offset 36
     * fee (uint24) - offset 68
     * recipient (address) - offset 100
     * deadline (uint256) - offset 132
     * amountIn (uint256) - offset 164
     * amountOutMinimum (uint256) - offset 196
     * sqrtPriceLimitX96 (uint160) - offset 228
     */

    mev_decode_address(calldata, calldata_len, 4, info->token_in);
    mev_decode_address(calldata, calldata_len, 36, info->token_out);
    
    /* Fee is at bytes 68-71 (uint24 right-aligned in 32 bytes) */
    info->fee = ((uint32_t)calldata[97] << 16) |
                ((uint32_t)calldata[98] << 8) |
                (uint32_t)calldata[99];

    mev_decode_uint256(calldata, calldata_len, 164, info->amount_in);
    mev_decode_uint256(calldata, calldata_len, 196, info->amount_out_min);

    return 0;
}

/**
 * Parse any swap type
 */
int mev_parse_swap(const uint8_t *calldata, size_t calldata_len,
                   mev_swap_info_t *info) {
    if (!calldata || !info || calldata_len < 4) {
        return -1;
    }

    memset(info, 0, sizeof(mev_swap_info_t));

    uint32_t selector = mev_parse_selector(calldata, calldata_len);

    switch (selector) {
        case SEL_SWAP_EXACT_TOKENS_V2:
        case SEL_SWAP_TOKENS_EXACT_V2:
            return mev_parse_v2_swap(calldata, calldata_len, info);
            
        case SEL_EXACT_INPUT_SINGLE_V3:
            return mev_parse_v3_swap(calldata, calldata_len, info);
            
        default:
            return -1;
    }
}
