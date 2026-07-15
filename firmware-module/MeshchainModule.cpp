#include "MeshchainModule.h"
#include <stdio.h>

MeshchainModule::MeshchainModule() {
    current_nonce = 1;
    selected_address_idx = 0;
    selected_amount = 10; // Default 10 MESH
}

void MeshchainModule::setup() {
    if (!crypto.load_keypair()) {
        crypto.generate_keypair();
    }
    
    // Add a dummy address for testing UI
    uint8_t dummy_bob[8] = {0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08};
    add_address("bob", dummy_bob);
}

void MeshchainModule::loop() {
    // Meshtastic background task hook
}

void MeshchainModule::onButtonPress() {
    // Simplistic UI logic: 
    // Short press cycles through addresses/amounts
    // Long press broadcasts Tx
    
    // For this mockup, we just trigger broadcast immediately
    broadcast_tx();
}

void MeshchainModule::drawScreen() {
    // Mock UI drawing logic using Meshtastic display APIs (e.g. u8g2)
    /*
    display->clearBuffer();
    display->setCursor(0, 10);
    display->print("Wallet:");
    
    // Draw current address selection
    if (!address_book.empty()) {
        display->setCursor(0, 20);
        display->printf("Send %llu MESH to %s", selected_amount, address_book[selected_address_idx].name.c_str());
    }
    
    // Draw last Tx History
    if (!tx_history.empty()) {
        display->setCursor(0, 40);
        display->printf("Last: -%llu to %s", tx_history.back().amount, tx_history.back().to_name.c_str());
    }
    display->sendBuffer();
    */
}

void MeshchainModule::add_address(const std::string& name, const uint8_t short_id[8]) {
    AddressBookEntry entry;
    entry.name = name;
    memcpy(entry.short_id, short_id, 8);
    address_book.push_back(entry);
}

void MeshchainModule::broadcast_tx() {
    if (address_book.empty()) return;
    
    Tx tx;
    tx.body.nonce = current_nonce++;
    crypto.get_short_id(tx.body.from_id);
    memcpy(tx.body.to_id, address_book[selected_address_idx].short_id, 8);
    tx.body.amount = selected_amount;
    tx.body.fee = 0;
    
    // Get payload to sign
    std::vector<uint8_t> msg = tx.body.encode();
    crypto.sign(msg, tx.signature);
    memcpy(tx.signer, crypto.get_public_key(), 32);
    
    std::vector<uint8_t> payload = tx.encode();
    
    // Hook into Meshtastic Router to broadcast
    // router->send(payload.data(), payload.size(), PORTNUM_MESHCHAIN);
    
    // Record in history
    TxHistoryEntry hist;
    hist.to_name = address_book[selected_address_idx].name;
    hist.amount = selected_amount;
    hist.timestamp = 0; // millis()
    tx_history.push_back(hist);
}
