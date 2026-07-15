#pragma once
#include <stdint.h>
#include <vector>

class CryptoEngine {
public:
    CryptoEngine();
    
    // Generates a new secp256k1 or ed25519 keypair and saves to NVS
    void generate_keypair();
    
    // Loads keypair from non-volatile storage
    bool load_keypair();
    
    // Signs a message hash (ed25519)
    void sign(const std::vector<uint8_t>& message, uint8_t signature_out[64]);
    
    // Getters for public identity
    const uint8_t* get_public_key() const { return public_key; }
    void get_short_id(uint8_t short_id_out[8]) const;

private:
    uint8_t private_key[32];
    uint8_t public_key[32];
};
