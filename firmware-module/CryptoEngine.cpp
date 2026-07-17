#include "CryptoEngine.h"
#include "ed25519_tiny.h"
#include <string.h>

#if defined(ESP32)
#include <esp_random.h>
#include <Preferences.h>
#else
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#endif

// ── Simple SHA-256 for short ID generation ───────────────────────────────────

struct SHA256_Context {
    uint32_t state[8];
    uint64_t count;
    uint8_t buffer[64];
};

static inline uint32_t rotr32(uint32_t x, uint32_t n) {
    return (x >> n) | (x << (32 - n));
}

static void sha256_transform(SHA256_Context *ctx, const uint8_t *data) {
    uint32_t a, b, c, d, e, f, g, h, i, j, t1, t2, m[64];

    for (i = 0, j = 0; i < 16; ++i, j += 4)
        m[i] = ((uint32_t)data[j] << 24) | ((uint32_t)data[j + 1] << 16) | ((uint32_t)data[j + 2] << 8) | ((uint32_t)data[j + 3]);
    for ( ; i < 64; ++i)
        m[i] = (rotr32(m[i - 2], 17) ^ rotr32(m[i - 2], 19) ^ (m[i - 2] >> 10)) + m[i - 7] + 
               (rotr32(m[i - 15], 7) ^ rotr32(m[i - 15], 18) ^ (m[i - 15] >> 3)) + m[i - 16];

    a = ctx->state[0];
    b = ctx->state[1];
    c = ctx->state[2];
    d = ctx->state[3];
    e = ctx->state[4];
    f = ctx->state[5];
    g = ctx->state[6];
    h = ctx->state[7];

    static const uint32_t k[64] = {
        0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
        0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
        0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
        0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
        0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
        0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
        0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
        0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
    };

    for (i = 0; i < 64; ++i) {
        t1 = h + (rotr32(e, 6) ^ rotr32(e, 11) ^ rotr32(e, 25)) + ((e & f) ^ (~e & g)) + k[i] + m[i];
        t2 = (rotr32(a, 2) ^ rotr32(a, 13) ^ rotr32(a, 22)) + ((a & b) ^ (a & c) ^ (b & c));
        h = g;
        g = f;
        f = e;
        e = d + t1;
        d = c;
        c = b;
        b = a;
        a = t1 + t2;
    }

    ctx->state[0] += a;
    ctx->state[1] += b;
    ctx->state[2] += c;
    ctx->state[3] += d;
    ctx->state[4] += e;
    ctx->state[5] += f;
    ctx->state[6] += g;
    ctx->state[7] += h;
}

static void sha256(const uint8_t *data, size_t len, uint8_t *hash) {
    SHA256_Context ctx;
    ctx.state[0] = 0x6a09e667;
    ctx.state[1] = 0xbb67ae85;
    ctx.state[2] = 0x3c6ef372;
    ctx.state[3] = 0xa54ff53a;
    ctx.state[4] = 0x510e527f;
    ctx.state[5] = 0x9b05688c;
    ctx.state[6] = 0x1f83d9ab;
    ctx.state[7] = 0x5be0cd19;
    ctx.count = 0;

    for (size_t i = 0; i < len; ++i) {
        ctx.buffer[ctx.count % 64] = data[i];
        ctx.count++;
        if (ctx.count % 64 == 0)
            sha256_transform(&ctx, ctx.buffer);
    }

    uint64_t bitlen = ctx.count * 8;
    uint32_t i = ctx.count % 64;
    if (i < 56) {
        ctx.buffer[i++] = 0x80;
        while (i < 56) ctx.buffer[i++] = 0x00;
    } else {
        ctx.buffer[i++] = 0x80;
        while (i < 64) ctx.buffer[i++] = 0x00;
        sha256_transform(&ctx, ctx.buffer);
        memset(ctx.buffer, 0, 56);
    }
    for (int j = 0; j < 8; ++j) {
        ctx.buffer[56 + j] = (bitlen >> (56 - j * 8)) & 0xFF;
    }
    sha256_transform(&ctx, ctx.buffer);

    for (i = 0; i < 4; ++i) {
        hash[i]      = (ctx.state[0] >> (24 - i * 8)) & 0xFF;
        hash[i + 4]  = (ctx.state[1] >> (24 - i * 8)) & 0xFF;
        hash[i + 8]  = (ctx.state[2] >> (24 - i * 8)) & 0xFF;
        hash[i + 12] = (ctx.state[3] >> (24 - i * 8)) & 0xFF;
        hash[i + 16] = (ctx.state[4] >> (24 - i * 8)) & 0xFF;
        hash[i + 20] = (ctx.state[5] >> (24 - i * 8)) & 0xFF;
        hash[i + 24] = (ctx.state[6] >> (24 - i * 8)) & 0xFF;
        hash[i + 28] = (ctx.state[7] >> (24 - i * 8)) & 0xFF;
    }
}

// ── CryptoEngine Implementation ──────────────────────────────────────────────

CryptoEngine::CryptoEngine() {
    memset(private_key, 0, 32);
    memset(public_key, 0, 32);
}

void CryptoEngine::generate_keypair() {
#if defined(ESP32)
    esp_fill_random(private_key, 32);
#elif defined(ARDUINO)
    static bool seeded = false;
    if (!seeded) {
        randomSeed(analogRead(0));
        seeded = true;
    }
    for (int i = 0; i < 32; i++) {
        private_key[i] = random(256);
    }
#else
    static bool seeded = false;
    if (!seeded) {
        srand(time(NULL));
        seeded = true;
    }
    for (int i = 0; i < 32; i++) {
        private_key[i] = rand() % 256;
    }
#endif

    // Generate standard Ed25519 public key
    ed25519_create_public_key(public_key, private_key);

    // Save keypair to persistent storage
#if defined(ESP32)
    Preferences prefs;
    prefs.begin("meshchain", false);
    prefs.putBytes("priv", private_key, 32);
    prefs.putBytes("pub", public_key, 32);
    prefs.end();
#else
    FILE* f = fopen("mock_nvs.bin", "wb");
    if (f) {
        fwrite(private_key, 1, 32, f);
        fwrite(public_key, 1, 32, f);
        fclose(f);
    }
#endif
}

bool CryptoEngine::load_keypair() {
#if defined(ESP32)
    Preferences prefs;
    prefs.begin("meshchain", false);
    size_t priv_len = prefs.getBytes("priv", private_key, 32);
    size_t pub_len = prefs.getBytes("pub", public_key, 32);
    prefs.end();
    return (priv_len == 32 && pub_len == 32);
#else
    FILE* f = fopen("mock_nvs.bin", "rb");
    if (!f) return false;
    size_t r1 = fread(private_key, 1, 32, f);
    size_t r2 = fread(public_key, 1, 32, f);
    fclose(f);
    return (r1 == 32 && r2 == 32);
#endif
}

void CryptoEngine::sign(const std::vector<uint8_t>& message, uint8_t signature_out[64]) {
    ed25519_sign(signature_out, message.data(), message.size(), public_key, private_key);
}

void CryptoEngine::get_short_id(uint8_t short_id_out[8]) const {
    uint8_t hash[32];
    sha256(public_key, 32, hash);
    memcpy(short_id_out, hash, 8);
}
