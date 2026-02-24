#ifndef MEV_PARSER_H
#define MEV_PARSER_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* DEX types */
typedef enum {
    DEX_UNKNOWN = 0,
    DEX_UNISWAP_V2 = 1,
    DEX_UNISWAP_V3 = 2,
    DEX_SUSHISWAP = 3,
    DEX_CURVE = 4,
    DEX_BALANCER = 5
} mev_dex_type_t;

/* Swap information extracted from calldata */
typedef struct {
    mev_dex_type_t dex_type;
    uint8_t token_in[20];
    uint8_t token_out[20];
    uint8_t amount_in[32];
    uint8_t amount_out_min[32];
    uint32_t fee;           /* Fee in hundredths of bip (V3) */
} mev_swap_info_t;

/**
 * Extract function selector from calldata
 */
uint32_t mev_parse_selector(const uint8_t *calldata, size_t calldata_len);

/**
 * Check if selector is a swap function
 */
int mev_is_swap_selector(uint32_t selector);

/**
 * Decode uint256 from calldata at offset
 */
int mev_decode_uint256(const uint8_t *calldata, size_t calldata_len,
                       size_t offset, uint8_t *value);

/**
 * Decode address from calldata at offset
 */
int mev_decode_address(const uint8_t *calldata, size_t calldata_len,
                       size_t offset, uint8_t *address);

/**
 * Parse UniswapV2 swap calldata
 */
int mev_parse_v2_swap(const uint8_t *calldata, size_t calldata_len,
                      mev_swap_info_t *info);

/**
 * Parse UniswapV3 exactInputSingle calldata
 */
int mev_parse_v3_swap(const uint8_t *calldata, size_t calldata_len,
                      mev_swap_info_t *info);

/**
 * Parse any supported swap type
 */
int mev_parse_swap(const uint8_t *calldata, size_t calldata_len,
                   mev_swap_info_t *info);

#ifdef __cplusplus
}
#endif

#endif /* MEV_PARSER_H */
