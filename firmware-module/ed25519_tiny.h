#pragma once
#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// Generates public key from a 32-byte private key seed.
// public_key must be 32 bytes, private_key must be 32 bytes.
void ed25519_create_public_key(uint8_t *public_key, const uint8_t *private_key);

// Signs a message of length message_len.
// signature must be 64 bytes, public_key 32 bytes, private_key 32 bytes.
void ed25519_sign(uint8_t *signature, const uint8_t *message, size_t message_len,
                  const uint8_t *public_key, const uint8_t *private_key);

// Verifies a signature (optional, for completeness/local tests).
int ed25519_verify(const uint8_t *signature, const uint8_t *message, size_t message_len,
                    const uint8_t *public_key);

#ifdef __cplusplus
}
#endif
