/**
 * MEV Protocol - C Hot Path Tests
 */

#include <stdio.h>
#include <string.h>
#include <assert.h>
#include "../include/keccak.h"
#include "../include/rlp.h"
#include "../include/parser.h"

/* Test colors */
#define GREEN "\033[32m"
#define RED "\033[31m"
#define RESET "\033[0m"

#define TEST(name) printf("  Testing %s... ", name)
#define PASS() printf(GREEN "PASS" RESET "\n")
#define FAIL() printf(RED "FAIL" RESET "\n")

void test_keccak256() {
    printf("\n=== Keccak256 Tests ===\n");

    /* Test 1: Empty input */
    TEST("empty input");
    {
        uint8_t output[32];
        uint8_t expected[] = {
            0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c,
            0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7, 0x03, 0xc0,
            0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b,
            0x7b, 0xfa, 0xd8, 0x04, 0x5d, 0x85, 0xa4, 0x70
        };
        
        mev_keccak256((uint8_t*)"", 0, output);
        assert(memcmp(output, expected, 32) == 0);
        PASS();
    }

    /* Test 2: "hello" */
    TEST("hello");
    {
        uint8_t output[32];
        uint8_t expected[] = {
            0x1c, 0x8a, 0xff, 0x95, 0x06, 0x85, 0xc2, 0xed,
            0x4b, 0xc3, 0x17, 0x4f, 0x34, 0x72, 0x28, 0x7b,
            0x56, 0xd9, 0x51, 0x7b, 0x9c, 0x94, 0x81, 0x27,
            0x31, 0x9a, 0x09, 0xa7, 0xa3, 0x6d, 0xea, 0xc8
        };
        
        mev_keccak256((uint8_t*)"hello", 5, output);
        assert(memcmp(output, expected, 32) == 0);
        PASS();
    }

    /* Test 3: Function selector */
    TEST("function selector");
    {
        uint32_t sel = mev_function_selector("transfer(address,uint256)");
        assert(sel == 0xa9059cbb);
        PASS();
    }
}

void test_rlp() {
    printf("\n=== RLP Tests ===\n");

    /* Test 1: Single byte */
    TEST("single byte < 0x80");
    {
        uint8_t input[] = {0x42};
        uint8_t output[2];
        size_t output_len;
        
        mev_rlp_encode_string(input, 1, output, &output_len);
        assert(output_len == 1);
        assert(output[0] == 0x42);
        PASS();
    }

    /* Test 2: Short string */
    TEST("short string");
    {
        uint8_t input[] = "dog";
        uint8_t output[10];
        size_t output_len;
        
        mev_rlp_encode_string(input, 3, output, &output_len);
        assert(output_len == 4);
        assert(output[0] == 0x83);
        assert(output[1] == 'd');
        assert(output[2] == 'o');
        assert(output[3] == 'g');
        PASS();
    }

    /* Test 3: Address encoding */
    TEST("address encoding");
    {
        uint8_t address[20] = {0xde, 0xad, 0xbe, 0xef};
        uint8_t output[22];
        size_t output_len;
        
        mev_rlp_encode_address(address, output, &output_len);
        assert(output_len == 21);
        assert(output[0] == 0x94);
        PASS();
    }
}

void test_parser() {
    printf("\n=== Parser Tests ===\n");

    /* Test 1: Selector extraction */
    TEST("selector extraction");
    {
        uint8_t calldata[] = {0x38, 0xed, 0x17, 0x39, 0x00};
        uint32_t sel = mev_parse_selector(calldata, 5);
        assert(sel == 0x38ed1739);
        PASS();
    }

    /* Test 2: Is swap selector */
    TEST("is swap selector");
    {
        assert(mev_is_swap_selector(0x38ed1739) == 1); /* UniV2 */
        assert(mev_is_swap_selector(0x414bf389) == 1); /* UniV3 */
        assert(mev_is_swap_selector(0x12345678) == 0); /* Unknown */
        PASS();
    }

    /* Test 3: Address decoding */
    TEST("address decoding");
    {
        uint8_t calldata[64] = {0};
        /* Address at offset 12 (right-aligned in 32 bytes) */
        calldata[12] = 0xde;
        calldata[13] = 0xad;
        calldata[14] = 0xbe;
        calldata[15] = 0xef;
        
        uint8_t address[20];
        mev_decode_address(calldata, 64, 0, address);
        
        assert(address[0] == 0xde);
        assert(address[1] == 0xad);
        assert(address[2] == 0xbe);
        assert(address[3] == 0xef);
        PASS();
    }
}

int main() {
    printf("MEV Protocol - C Hot Path Test Suite\n");
    printf("=====================================\n");

    test_keccak256();
    test_rlp();
    test_parser();

    printf("\n" GREEN "All tests passed!" RESET "\n\n");
    return 0;
}
