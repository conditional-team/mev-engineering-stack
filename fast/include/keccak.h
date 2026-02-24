#ifndef MEV_KECCAK_H
#define MEV_KECCAK_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Compute Keccak-256 hash
 * 
 * @param input Input data
 * @param input_len Length of input data  
 * @param output Output buffer (must be at least 32 bytes)
 * @return 0 on success, -1 on error
 */
int mev_keccak256(const uint8_t *input, size_t input_len, uint8_t *output);

/**
 * Compute Keccak-256 for Ethereum address derivation
 * 
 * @param pubkey Public key bytes (64 bytes, uncompressed without 0x04 prefix)
 * @param pubkey_len Length of public key
 * @param address Output buffer (must be at least 20 bytes)
 * @return 0 on success, -1 on error
 */
int mev_keccak256_address(const uint8_t *pubkey, size_t pubkey_len, uint8_t *address);

/**
 * Compute Solidity function selector
 * 
 * @param signature Function signature, e.g. "transfer(address,uint256)"
 * @return 4-byte selector as uint32
 */
uint32_t mev_function_selector(const char *signature);

#ifdef __cplusplus
}
#endif

#endif /* MEV_KECCAK_H */
