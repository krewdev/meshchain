#include "BincodeTx.h"
#include <iostream>
#include <iomanip>

int main() {
    Tx tx;
    tx.body.nonce = 42;
    tx.body.amount = 1000;
    tx.body.fee = 50;
    
    uint8_t from[8] = {1, 2, 3, 4, 5, 6, 7, 8};
    uint8_t to[8] = {8, 7, 6, 5, 4, 3, 2, 1};
    memcpy(tx.body.from_id, from, 8);
    memcpy(tx.body.to_id, to, 8);
    
    memset(tx.signature, 0xAA, 64);
    memset(tx.signer, 0xBB, 32);
    
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
