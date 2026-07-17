#include "BincodeTx.h"
#include "CryptoEngine.h"
#include <iostream>
#include <iomanip>

int main() {
    std::cout << "Initializing CryptoEngine and generating keypair...\n";
    CryptoEngine crypto;
    crypto.generate_keypair();
    
    CryptoEngine crypto2;
    if (!crypto2.load_keypair()) {
        std::cout << "❌ Failed to load keypair from mock NVS!\n";
        return 1;
    }
    std::cout << "✅ Keypair successfully saved and loaded from mock NVS.\n";
    
    std::cout << "Public Key: ";
    const uint8_t* pk = crypto.get_public_key();
    for (int i = 0; i < 32; ++i) {
        std::cout << std::hex << std::setw(2) << std::setfill('0') << (int)pk[i];
    }
    std::cout << std::dec << "\n";

    uint8_t short_id[8];
    crypto.get_short_id(short_id);
    std::cout << "Short ID: ";
    for (int i = 0; i < 8; ++i) {
        std::cout << std::hex << std::setw(2) << std::setfill('0') << (int)short_id[i];
    }
    std::cout << std::dec << "\n";

    Tx tx;
    tx.body.nonce = 42;
    tx.body.amount = 1000;
    tx.body.fee = 50;
    
    memcpy(tx.body.from_id, short_id, 8);
    uint8_t to[8] = {8, 7, 6, 5, 4, 3, 2, 1};
    memcpy(tx.body.to_id, to, 8);
    
    std::vector<uint8_t> msg = tx.body.encode();
    crypto.sign(msg, tx.signature);
    memcpy(tx.signer, crypto.get_public_key(), 32);
    
    std::vector<uint8_t> out = tx.encode();
    
    std::cout << "Encoded Tx length: " << out.size() << " bytes\n";
    std::cout << "Expected length: 138 bytes\n\n";
    
    std::cout << "Hex dump:\n";
    for (size_t i = 0; i < out.size(); ++i) {
        std::cout << std::hex << std::setw(2) << std::setfill('0') << (int)out[i] << " ";
        if ((i + 1) % 16 == 0) std::cout << "\n";
    }
    std::cout << std::dec << "\n";
    
    if (out.size() == 138) {
        std::cout << "✅ C++ Bincode encoding is structurally correct.\n";
    } else {
        std::cout << "❌ Encoding size mismatch.\n";
        return 1;
    }
    
    return 0;
}
