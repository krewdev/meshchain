#include "CryptoEngine.h"
#include <string.h>

// Placeholder for uECC / mbedtls

CryptoEngine::CryptoEngine() {
    memset(private_key, 0, 32);
    memset(public_key, 0, 32);
}

void CryptoEngine::generate_keypair() {
    // TODO: use hardware RNG to generate 32 bytes
    // uECC_make_key(public_key, private_key, uECC_secp256k1());
}

bool CryptoEngine::load_keypair() {
    // TODO: read from ESP32 Preferences/NVS
    return false;
}

void CryptoEngine::sign(const std::vector<uint8_t>& message, uint8_t signature_out[64]) {
    // TODO: implement ed25519 or ECDSA signature using micro-ecc
    // uECC_sign(private_key, message.data(), message.size(), signature_out, uECC_secp256k1());
    memset(signature_out, 1, 64); // dummy signature
}

void CryptoEngine::get_short_id(uint8_t short_id_out[8]) const {
    // TODO: BLAKE3 or SHA256 trunc 8 of public key
    memcpy(short_id_out, public_key, 8);
}
