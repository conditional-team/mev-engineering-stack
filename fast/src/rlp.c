/**
 * MEV Protocol - C Hot Path
 * Fast RLP Encoding/Decoding
 */

#include "rlp.h"
#include <string.h>

/**
 * Encode single byte
 */
static size_t encode_byte(uint8_t byte, uint8_t *output) {
    if (byte < 0x80) {
        output[0] = byte;
        return 1;
    } else {
        output[0] = 0x81;
        output[1] = byte;
        return 2;
    }
}

/**
 * Encode length prefix
 */
static size_t encode_length(size_t len, uint8_t offset, uint8_t *output) {
    if (len < 56) {
        output[0] = (uint8_t)(offset + len);
        return 1;
    } else {
        /* Calculate bytes needed for length */
        size_t len_bytes = 0;
        size_t temp = len;
        while (temp > 0) {
            len_bytes++;
            temp >>= 8;
        }
        
        output[0] = (uint8_t)(offset + 55 + len_bytes);
        
        /* Write length in big endian */
        for (size_t i = len_bytes; i > 0; i--) {
            output[i] = (uint8_t)(len & 0xff);
            len >>= 8;
        }
        
        return 1 + len_bytes;
    }
}

/**
 * RLP encode a byte string
 */
int mev_rlp_encode_string(const uint8_t *input, size_t input_len, 
                          uint8_t *output, size_t *output_len) {
    if (!input || !output || !output_len) {
        return -1;
    }

    size_t offset = 0;

    if (input_len == 1 && input[0] < 0x80) {
        /* Single byte, no prefix */
        output[0] = input[0];
        *output_len = 1;
        return 0;
    }

    if (input_len < 56) {
        /* Short string: 0x80 + len */
        output[0] = (uint8_t)(0x80 + input_len);
        offset = 1;
    } else {
        /* Long string: 0xb7 + len_of_len + len */
        offset = encode_length(input_len, 0xb7, output);
    }

    memcpy(output + offset, input, input_len);
    *output_len = offset + input_len;
    
    return 0;
}

/**
 * RLP encode a list
 */
int mev_rlp_encode_list(const uint8_t *payload, size_t payload_len,
                        uint8_t *output, size_t *output_len) {
    if (!payload || !output || !output_len) {
        return -1;
    }

    size_t offset;

    if (payload_len < 56) {
        /* Short list: 0xc0 + len */
        output[0] = (uint8_t)(0xc0 + payload_len);
        offset = 1;
    } else {
        /* Long list: 0xf7 + len_of_len + len */
        offset = encode_length(payload_len, 0xf7, output);
    }

    memcpy(output + offset, payload, payload_len);
    *output_len = offset + payload_len;
    
    return 0;
}

/**
 * RLP encode a uint256
 */
int mev_rlp_encode_uint256(const uint8_t *value, uint8_t *output, size_t *output_len) {
    if (!value || !output || !output_len) {
        return -1;
    }

    /* Find first non-zero byte */
    size_t start = 0;
    while (start < 32 && value[start] == 0) {
        start++;
    }

    if (start == 32) {
        /* Zero value */
        output[0] = 0x80;
        *output_len = 1;
        return 0;
    }

    size_t len = 32 - start;
    
    return mev_rlp_encode_string(value + start, len, output, output_len);
}

/**
 * RLP encode an Ethereum address (20 bytes)
 */
int mev_rlp_encode_address(const uint8_t *address, uint8_t *output, size_t *output_len) {
    if (!address || !output || !output_len) {
        return -1;
    }

    /* Address is always 20 bytes, prefix is 0x80 + 20 = 0x94 */
    output[0] = 0x94;
    memcpy(output + 1, address, 20);
    *output_len = 21;
    
    return 0;
}

/**
 * Decode RLP string
 */
int mev_rlp_decode_string(const uint8_t *input, size_t input_len,
                          const uint8_t **data, size_t *data_len, size_t *consumed) {
    if (!input || !data || !data_len || !consumed || input_len == 0) {
        return -1;
    }

    uint8_t prefix = input[0];

    if (prefix < 0x80) {
        /* Single byte */
        *data = input;
        *data_len = 1;
        *consumed = 1;
        return 0;
    }

    if (prefix <= 0xb7) {
        /* Short string */
        size_t len = prefix - 0x80;
        if (input_len < 1 + len) {
            return -1;
        }
        *data = input + 1;
        *data_len = len;
        *consumed = 1 + len;
        return 0;
    }

    if (prefix <= 0xbf) {
        /* Long string */
        size_t len_bytes = prefix - 0xb7;
        if (input_len < 1 + len_bytes) {
            return -1;
        }
        
        size_t len = 0;
        for (size_t i = 0; i < len_bytes; i++) {
            len = (len << 8) | input[1 + i];
        }
        
        if (input_len < 1 + len_bytes + len) {
            return -1;
        }
        
        *data = input + 1 + len_bytes;
        *data_len = len;
        *consumed = 1 + len_bytes + len;
        return 0;
    }

    /* It's a list, not a string */
    return -1;
}

/**
 * Get total encoded length of a value
 */
size_t mev_rlp_encoded_length(size_t data_len) {
    if (data_len == 1) {
        return 1; /* May be single byte or prefixed */
    } else if (data_len < 56) {
        return 1 + data_len;
    } else {
        size_t len_bytes = 0;
        size_t temp = data_len;
        while (temp > 0) {
            len_bytes++;
            temp >>= 8;
        }
        return 1 + len_bytes + data_len;
    }
}
