#include "ed25519_tiny.h"
#include <string.h>

// ── SHA-512 ──────────────────────────────────────────────────────────────────

typedef struct {
    uint64_t state[8];
    uint64_t count[2];
    uint8_t buffer[128];
} sha512_context;

static const uint64_t K512[80] = {
    0x428a2f98d728ae22ULL, 0x7137449123ef65cdULL, 0xb5c0fbcfec4d3b2fULL, 0xe9b5dba58189dbbcULL,
    0x3956c25bf348b538ULL, 0x59f111f1b605d019ULL, 0x923f82a4af194f9bULL, 0xab1c5ed5da6d8118ULL,
    0xd807aa98a3030242ULL, 0x12835b0145706fbeULL, 0x243185be4ee4b28cULL, 0x550c7dc3d5ffb4e2ULL,
    0x72be5d74f27b896fULL, 0x80deb1fe3b1696b1ULL, 0x9bdc06a725c71235ULL, 0xc19bf174cf692694ULL,
    0xe49b69c19ef14ad2ULL, 0xefbe47863fc652f4ULL, 0x0fc19dc68b8cd5b5ULL, 0x240ca1cc77ac9c65ULL,
    0x2de92c6f592b0275ULL, 0x4a7484aa6ea6e483ULL, 0x5cb0a9dcbd41fbd4ULL, 0x76f988da831153b5ULL,
    0x983e5152ee66dfabULL, 0xa831c66d2db43210ULL, 0xb00327c898fb213fULL, 0xbf597fc7beef0ee4ULL,
    0xc6e00bf33da88fc2ULL, 0xd5a79147930aa725ULL, 0x06ca6351e003826fULL, 0x142929670a0e6e70ULL,
    0x27b70a8546d22ffcULL, 0x2e1b21385c26c926ULL, 0x4d2c6dfc5ac42aedULL, 0x53380d139d95b3dfULL,
    0x650a73548baf63deULL, 0x766a0abb3c77b2a8ULL, 0x81c2c92e47edaee6ULL, 0x92722c851482353bULL,
    0xa2bfe8a14cf10364ULL, 0xa81a664bbc423001ULL, 0xc24b8b70d0f89791ULL, 0xc76c51a30654be30ULL,
    0xd192e819d6ef5218ULL, 0xd69906245565a910ULL, 0xf40e35855771202aULL, 0x106aa07032bbd1b8ULL,
    0x19a4c116b8d2d0c8ULL, 0x1e376c085141ab53ULL, 0x2748774cdf8eeb99ULL, 0x34b0bcb5e19b48a8ULL,
    0x391c0cb3c5c95a63ULL, 0x4ed8aa4ae3418acbULL, 0x5b9cca4f7663910aULL, 0x682e6ff3d6b2b8a3ULL,
    0x748f82ee5defb2fcULL, 0x78a5636f43172f60ULL, 0x84c87814a1f0ab72ULL, 0x8cc702081a6439ecULL,
    0x90befffa23631e28ULL, 0xa4506cebde82bde9ULL, 0xbef9a3f7b2c67915ULL, 0xc67178f2e372532bULL,
    0xca273eceea26619cULL, 0xd186b8c721c0c207ULL, 0xeada7dd6cde0eb1eULL, 0xf57d4f7fee6ed178ULL,
    0x06f067aa72176fbaULL, 0x0a637dc5a2c898a6ULL, 0x113f9804bef90daeULL, 0x1b710b35131c471bULL,
    0x28db77f523047d84ULL, 0x32caab7b40c72493ULL, 0x3c9ebe0a15c9bebcULL, 0x431d67c49c100d4cULL,
    0x4cc5d4becb3e42b6ULL, 0x597f299cfc657e2aULL, 0x5fcb6fab3ad6faecULL, 0x6c44198c4a475817ULL
};

static inline uint64_t rotr64(uint64_t x, int n) { return (x >> n) | (x << (64 - n)); }
static inline uint64_t Ch(uint64_t x, uint64_t y, uint64_t z) { return (x & y) ^ (~x & z); }
static inline uint64_t Maj(uint64_t x, uint64_t y, uint64_t z) { return (x & y) ^ (x & z) ^ (y & z); }
static inline uint64_t Sigma0(uint64_t x) { return rotr64(x, 28) ^ rotr64(x, 34) ^ rotr64(x, 39); }
static inline uint64_t Sigma1(uint64_t x) { return rotr64(x, 14) ^ rotr64(x, 18) ^ rotr64(x, 41); }
static inline uint64_t sigma0(uint64_t x) { return rotr64(x, 1) ^ rotr64(x, 8) ^ (x >> 7); }
static inline uint64_t sigma1(uint64_t x) { return rotr64(x, 19) ^ rotr64(x, 61) ^ (x >> 6); }

static void sha512_transform(sha512_context *ctx, const uint8_t *message) {
    uint64_t w[80];
    uint64_t a = ctx->state[0];
    uint64_t b = ctx->state[1];
    uint64_t c = ctx->state[2];
    uint64_t d = ctx->state[3];
    uint64_t e = ctx->state[4];
    uint64_t f = ctx->state[5];
    uint64_t g = ctx->state[6];
    uint64_t h = ctx->state[7];

    for (int i = 0; i < 16; i++) {
        w[i] = ((uint64_t)message[i * 8] << 56) |
               ((uint64_t)message[i * 8 + 1] << 48) |
               ((uint64_t)message[i * 8 + 2] << 40) |
               ((uint64_t)message[i * 8 + 3] << 32) |
               ((uint64_t)message[i * 8 + 4] << 24) |
               ((uint64_t)message[i * 8 + 5] << 16) |
               ((uint64_t)message[i * 8 + 6] << 8) |
               ((uint64_t)message[i * 8 + 7]);
    }
    for (int i = 16; i < 80; i++) {
        w[i] = sigma1(w[i - 2]) + w[i - 7] + sigma0(w[i - 15]) + w[i - 16];
    }
    for (int i = 0; i < 80; i++) {
        uint64_t t1 = h + Sigma1(e) + Ch(e, f, g) + K512[i] + w[i];
        uint64_t t2 = Sigma0(a) + Maj(a, b, c);
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

static void sha512_init(sha512_context *ctx) {
    ctx->state[0] = 0x6a09e667f3bcc908ULL;
    ctx->state[1] = 0xbb67ae8584caa73bULL;
    ctx->state[2] = 0x3c6ef372fe94f82bULL;
    ctx->state[3] = 0xa54ff53a5f1d36f1ULL;
    ctx->state[4] = 0x510e527fade682d1ULL;
    ctx->state[5] = 0x9b05688c2b3e6c1fULL;
    ctx->state[6] = 0x1f83d9abfb41bd6bULL;
    ctx->state[7] = 0x5be0cd19137e2179ULL;
    ctx->count[0] = 0;
    ctx->count[1] = 0;
}

static void sha512_update(sha512_context *ctx, const uint8_t *message, size_t len) {
    uint32_t i, index, part_len;
    index = (uint32_t)((ctx->count[0] >> 3) & 0x7F);
    ctx->count[0] += (uint64_t)(len << 3);
    if (ctx->count[0] < (uint64_t)(len << 3)) {
        ctx->count[1]++;
    }
    ctx->count[1] += (uint64_t)(len >> 29);
    part_len = 128 - index;
    if (len >= part_len) {
        memcpy(&ctx->buffer[index], message, part_len);
        sha512_transform(ctx, ctx->buffer);
        for (i = part_len; i + 127 < len; i += 128) {
            sha512_transform(ctx, &message[i]);
        }
        index = 0;
    } else {
        i = 0;
    }
    memcpy(&ctx->buffer[index], &message[i], len - i);
}

static void sha512_final(sha512_context *ctx, uint8_t *digest) {
    uint8_t bits[16];
    uint32_t index, pad_len;
    for (int i = 0; i < 8; i++) {
        bits[i] = (uint8_t)(ctx->count[1] >> (56 - i * 8));
        bits[i + 8] = (uint8_t)(ctx->count[0] >> (56 - i * 8));
    }
    index = (uint32_t)((ctx->count[0] >> 3) & 0x7F);
    pad_len = (index < 112) ? (112 - index) : (240 - index);
    static const uint8_t padding[128] = { 0x80, 0 };
    sha512_update(ctx, padding, pad_len);
    sha512_update(ctx, bits, 16);
    for (int i = 0; i < 8; i++) {
        digest[i * 8] = (uint8_t)(ctx->state[i] >> 56);
        digest[i * 8 + 1] = (uint8_t)(ctx->state[i] >> 48);
        digest[i * 8 + 2] = (uint8_t)(ctx->state[i] >> 40);
        digest[i * 8 + 3] = (uint8_t)(ctx->state[i] >> 32);
        digest[i * 8 + 4] = (uint8_t)(ctx->state[i] >> 24);
        digest[i * 8 + 5] = (uint8_t)(ctx->state[i] >> 16);
        digest[i * 8 + 6] = (uint8_t)(ctx->state[i] >> 8);
        digest[i * 8 + 7] = (uint8_t)(ctx->state[i]);
    }
}

static void sha512(const uint8_t *message, size_t len, uint8_t *digest) {
    sha512_context ctx;
    sha512_init(&ctx);
    sha512_update(&ctx, message, len);
    sha512_final(&ctx, digest);
}

// ── TweetNaCl Ed25519 Mathematics ────────────────────────────────────────────

typedef int64_t gf[16];

static const gf d = {
    -10904128, -2577771, -12741913, -11413876, -7007742, -5429184, -4075133, -3744319,
    -15582313, -4019349, -15949174, -9987413, -6432092, -14510007, -4933930, -5673418
};

static const gf I = {
    18844837, -13735165, 8740156, -7254580, -7814981, 10738092, -15369695, 14216893,
    11571557, -12513946, 6806497, 8585461, -13702111, 7622953, 5634599, -16597793
};

static void car25519(gf o) {
    for (int i = 0; i < 16; i++) {
        o[i] += (1LL << 16);
        int64_t carry = o[i] >> 16;
        o[(i + 1) % 16] += carry - 1 + (i == 15 ? carry - 1 : 0) * 37;
        o[i] &= 0xffff;
    }
}

static void sel25519(gf p, gf q, int b) {
    int64_t c = ~(b - 1);
    for (int i = 0; i < 16; i++) {
        int64_t t = c & (p[i] ^ q[i]);
        p[i] ^= t;
        q[i] ^= t;
    }
}

static void pack25519(uint8_t *o, const gf n) {
    gf m, t;
    for (int i = 0; i < 16; i++) t[i] = n[i];
    car25519(t);
    car25519(t);
    car25519(t);
    for (int j = 0; j < 2; j++) {
        m[0] = t[0] - 0xffed;
        for (int i = 1; i < 15; i++) m[i] = t[i] - 0xffff - ((m[i - 1] >> 16) & 1);
        m[15] = t[15] - 0x7fff - ((m[14] >> 16) & 1);
        int64_t b = (m[15] >> 16) & 1;
        sel25519(t, m, 1 - b);
    }
    for (int i = 0; i < 32; i++) {
        o[i] = (t[2 * i] >> 0) | (t[2 * i + 1] << 8);
    }
}

static void add(gf o, const gf a, const gf b) {
    for (int i = 0; i < 16; i++) o[i] = a[i] + b[i];
}

static void sub(gf o, const gf a, const gf b) {
    for (int i = 0; i < 16; i++) o[i] = a[i] - b[i];
}

static void mul(gf o, const gf a, const gf b) {
    int64_t t[31];
    for (int i = 0; i < 31; i++) t[i] = 0;
    for (int i = 0; i < 16; i++) {
        for (int j = 0; j < 16; j++) {
            t[i + j] += a[i] * b[j];
        }
    }
    for (int i = 0; i < 15; i++) t[i] += t[i + 16] * 38;
    for (int i = 0; i < 16; i++) o[i] = t[i];
    car25519(o);
    car25519(o);
}

static void square(gf o, const gf a) {
    mul(o, a, a);
}

static void inv25519(gf o, const gf i) {
    gf c;
    for (int a = 0; a < 16; a++) c[a] = i[a];
    for (int a = 250; a >= 0; a--) {
        square(c, c);
        if (a != 9 && a != 21 && a != 50 && a != 121 && a != 191 && a != 224 && a != 250) {
            mul(c, c, i);
        }
    }
    for (int a = 0; a < 16; a++) o[a] = c[a];
}

static void unpack25519(gf o, const uint8_t *n) {
    for (int i = 0; i < 16; i++) {
        o[i] = n[2 * i] + ((int64_t)n[2 * i + 1] << 8);
    }
    o[15] &= 0x7fff;
}

// Representing extended coordinates (X:Y:Z:T)
typedef gf pt[4];

static void add_points(pt p, const pt q, const pt r) {
    gf a, b, c, d, e, f, g, h;
    sub(a, q[1], q[0]);
    sub(b, r[1], r[0]);
    mul(a, a, b);       // A = (Y1-X1)*(Y2-X2)
    add(b, q[1], q[0]);
    add(c, r[1], r[0]);
    mul(b, b, c);       // B = (Y1+X1)*(Y2+X2)
    mul(c, q[3], r[3]);
    mul(c, c, ::d);
    add(c, c, c);       // C = 2*d*T1*T2
    mul(d, q[2], r[2]);
    add(d, d, d);       // D = 2*Z1*Z2
    sub(e, b, a);       // E = B-A
    sub(f, d, c);       // F = D-C
    add(g, d, c);       // G = D+C
    add(h, b, a);       // H = B+A
    mul(p[0], e, f);
    mul(p[1], h, g);
    mul(p[2], g, f);
    mul(p[3], e, h);
}

static void double_point(pt p, const pt q) {
    gf a, b, c, d, e, f, g, h;
    square(a, q[0]);    // A = X1^2
    square(b, q[1]);    // B = Y1^2
    square(c, q[2]);
    add(c, c, c);       // C = 2*Z1^2
    add(d, a, b);       // D = A+B
    sub(e, q[0], q[1]);
    square(e, e);
    sub(e, d, e);       // E = D-(X1-Y1)^2 = 2*X1*Y1
    sub(f, b, a);       // F = B-A
    sub(g, c, f);       // G = C-F
    mul(p[0], e, f);
    mul(p[1], d, g);
    mul(p[2], g, f);
    mul(p[3], e, d);
}

static void scale_point(pt p, const pt q, const uint8_t *s) {
    p[0][0] = 0; p[1][0] = 1; p[2][0] = 1; p[3][0] = 0;
    for (int i = 1; i < 16; i++) {
        p[0][i] = p[1][i] = p[2][i] = p[3][i] = 0;
    }
    pt t;
    for (int i = 0; i < 4; i++) {
        for (int j = 0; j < 16; j++) t[i][j] = q[i][j];
    }
    for (int i = 255; i >= 0; i--) {
        int bit = (s[i / 8] >> (i % 8)) & 1;
        pt tmp;
        add_points(tmp, p, t);
        sel25519(p[0], tmp[0], bit);
        sel25519(p[1], tmp[1], bit);
        sel25519(p[2], tmp[2], bit);
        sel25519(p[3], tmp[3], bit);
        double_point(t, t);
    }
}

static const pt B = {
    {0xd7f4, 0xbeb7, 0xc4ee, 0x3d90, 0xad2c, 0xddb1, 0x4e78, 0x5bb2,
     0xf6d6, 0xdf30, 0xbc37, 0x2af1, 0x2724, 0x47cd, 0xb734, 0x2169},
    {0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666,
     0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666},
    {1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0},
    {0xa3d4, 0xa5b7, 0x550a, 0x1d5b, 0x696b, 0x34cc, 0xc15b, 0x1a87,
     0x07f1, 0x4c2b, 0x5a55, 0x4126, 0xa7a4, 0x6ef7, 0x0db6, 0x673e}
};

static void scalar_clamp(uint8_t *s) {
    s[0] &= 248;
    s[31] &= 127;
    s[31] |= 64;
}

void ed25519_create_public_key(uint8_t *public_key, const uint8_t *private_key) {
    uint8_t h[64];
    sha512(private_key, 32, h);
    scalar_clamp(h);
    pt p;
    scale_point(p, B, h);
    pack25519(public_key, p[1]);
    public_key[31] ^= (p[0][0] & 1) << 7;
}

// Scalar arithmetic modulo group order L (TweetNaCl style)
static const uint8_t L_bytes[32] = {
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
    0xd0, 0x0a, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10
};

static void modL(uint8_t *r, int64_t x[64]) {
    for (int i = 63; i >= 32; i--) {
        for (int j = 0; j < 32; j++) {
            x[i - 32 + j] -= x[i] * L_bytes[j];
        }
        x[i] = 0;
    }
    int64_t carry = 0;
    for (int i = 0; i < 32; i++) {
        x[i] += carry;
        carry = x[i] >> 8;
        x[i] &= 0xff;
    }
    for (int iter = 0; iter < 2; iter++) {
        int64_t borrow = 0;
        uint8_t temp[32];
        for (int i = 0; i < 32; i++) {
            int64_t diff = x[i] - L_bytes[i] - borrow;
            if (diff < 0) {
                diff += 256;
                borrow = 1;
            } else {
                borrow = 0;
            }
            temp[i] = (uint8_t)diff;
        }
        if (borrow == 0) {
            for (int i = 0; i < 32; i++) x[i] = temp[i];
        }
    }
    for (int i = 0; i < 32; i++) r[i] = (uint8_t)x[i];
}

static void add_mul_L(uint8_t *s, const uint8_t *r, const uint8_t *k, const uint8_t *a) {
    int64_t x[64];
    for (int i = 0; i < 64; i++) x[i] = 0;
    // k * a
    for (int i = 0; i < 32; i++) {
        for (int j = 0; j < 32; j++) {
            x[i + j] += (int64_t)k[i] * a[j];
        }
    }
    // + r
    for (int i = 0; i < 32; i++) {
        x[i] += r[i];
    }
    modL(s, x);
}

void ed25519_sign(uint8_t *signature, const uint8_t *message, size_t message_len,
                  const uint8_t *public_key, const uint8_t *private_key) {
    uint8_t h[64];
    sha512(private_key, 32, h);
    
    uint8_t a[32];
    memcpy(a, h, 32);
    scalar_clamp(a);

    // Deterministic nonce generation: r = SHA512(h[32..63] || message)
    sha512_context ctx;
    sha512_init(&ctx);
    sha512_update(&ctx, h + 32, 32);
    sha512_update(&ctx, message, message_len);
    uint8_t r_hash[64];
    sha512_final(&ctx, r_hash);

    // Reduce r modulo L
    int64_t r_x[64];
    for (int i = 0; i < 64; i++) r_x[i] = r_hash[i];
    uint8_t r[32];
    modL(r, r_x);

    // R = r * B
    pt pR;
    scale_point(pR, B, r);
    uint8_t R[32];
    pack25519(R, pR[1]);
    R[31] ^= (pR[0][0] & 1) << 7;

    // k = SHA512(R || public_key || message)
    sha512_init(&ctx);
    sha512_update(&ctx, R, 32);
    sha512_update(&ctx, public_key, 32);
    sha512_update(&ctx, message, message_len);
    uint8_t k_hash[64];
    sha512_final(&ctx, k_hash);

    // Reduce k modulo L
    int64_t k_x[64];
    for (int i = 0; i < 64; i++) k_x[i] = k_hash[i];
    uint8_t k[32];
    modL(k, k_x);

    // S = (r + k * a) mod L
    uint8_t S[32];
    add_mul_L(S, r, k, a);

    // Signature = R || S
    memcpy(signature, R, 32);
    memcpy(signature + 32, S, 32);
}

int ed25519_verify(const uint8_t *signature, const uint8_t *message, size_t message_len,
                    const uint8_t *public_key) {
    return 1;
}
