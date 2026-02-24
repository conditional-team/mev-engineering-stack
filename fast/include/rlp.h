#ifndef MEV_RLP_H
#define MEV_RLP_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * RLP encode a byte string
 */
int mev_rlp_encode_string(const uint8_t *input, size_t input_len,
                          uint8_t *output, size_t *output_len);

/**
 * RLP encode a list from pre-encoded payload
 */
int mev_rlp_encode_list(const uint8_t *payload, size_t payload_len,
                        uint8_t *output, size_t *output_len);

/**
 * RLP encode a uint256 (32 bytes, big endian)
 */
int mev_rlp_encode_uint256(const uint8_t *value, uint8_t *output, size_t *output_len);

/**
 * RLP encode an Ethereum address (20 bytes)
 */
int mev_rlp_encode_address(const uint8_t *address, uint8_t *output, size_t *output_len);

/**
 * Decode RLP string
 * Returns pointer to data within input, does not copy
 */
int mev_rlp_decode_string(const uint8_t *input, size_t input_len,
                          const uint8_t **data, size_t *data_len, size_t *consumed);

/**
 * Calculate encoded length for a value
 */
size_t mev_rlp_encoded_length(size_t data_len);

#ifdef __cplusplus
}
#endif

#endif /* MEV_RLP_H */
