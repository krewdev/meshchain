#pragma once
#include <stdint.h>
#include <string.h>
#include <vector>

// C++ equivalent of Meshchain's Bincode v1 Serialization for TxBody::Transfer
struct TxBodyTransfer {
    uint32_t nonce;
    uint8_t from_id[8];
    uint8_t to_id[8];
    uint64_t amount;
    uint64_t fee;

    std::vector<uint8_t> encode() const {
        std::vector<uint8_t> buf(40, 0);
        uint32_t variant = 0; // Transfer = 0
        memcpy(buf.data() + 0, &variant, 4);
        memcpy(buf.data() + 4, &nonce, 4);
        memcpy(buf.data() + 8, from_id, 8);
        memcpy(buf.data() + 16, to_id, 8);
        memcpy(buf.data() + 24, &amount, 8);
        memcpy(buf.data() + 32, &fee, 8);
        return buf;
    }
};

struct Tx {
    TxBodyTransfer body;
    uint8_t signature[64];
    uint8_t signer[32];

    std::vector<uint8_t> encode() const {
        std::vector<uint8_t> body_bytes = body.encode();
        std::vector<uint8_t> buf;
        buf.reserve(body_bytes.size() + 64 + 32 + 2);
        
        buf.insert(buf.end(), body_bytes.begin(), body_bytes.end());
        buf.insert(buf.end(), signature, signature + 64);
        buf.insert(buf.end(), signer, signer + 32);
        
        // Option<Vec<u8>> is represented as a single byte 0x00 when None
        buf.push_back(0x00); // pq_pk = None
        buf.push_back(0x00); // pq_sig = None
        
        return buf;
    }
};
