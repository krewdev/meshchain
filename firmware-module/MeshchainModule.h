#pragma once

#include "CryptoEngine.h"
#include "BincodeTx.h"
#include <string>
#include <vector>

// Forward declarations for Meshtastic framework hooks
namespace meshtastic {
    class PluginModule;
}

struct AddressBookEntry {
    std::string name;
    uint8_t short_id[8];
};

struct TxHistoryEntry {
    std::string to_name;
    uint64_t amount;
    uint32_t timestamp;
};

class MeshchainModule {
public:
    MeshchainModule();
    void setup();
    void loop();
    
    // UI Hooks
    void onButtonPress();
    void drawScreen();
    
    // Address Book Management
    void add_address(const std::string& name, const uint8_t short_id[8]);
    
private:
    CryptoEngine crypto;
    uint32_t current_nonce;
    
    std::vector<AddressBookEntry> address_book;
    std::vector<TxHistoryEntry> tx_history;
    
    // UI State
    int selected_address_idx;
    uint64_t selected_amount;
    
    void broadcast_tx();
};
