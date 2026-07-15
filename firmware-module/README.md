# Meshchain Hardware Wallet Module

This directory contains the C++ source code required to turn a Meshtastic LoRa node (ESP32 or nRF52) into a standalone hardware wallet for Meshchain.

## Features
- **Standalone Tx Generation**: Creates perfectly formatted Bincode-compliant transactions.
- **On-Device Cryptography**: Uses `micro-ecc` (or `mbedtls`) to generate ECDSA signatures, keeping your private key securely in the device's flash memory.
- **Custom UI**: Integrates with the Meshtastic OLED screen, allowing you to cycle through an Address Book and send transactions using the physical buttons.
- **Direct Broadcasting**: Pushes transactions directly over the LoRa mesh via the Meshtastic Router.

## Integration Instructions

Because this relies on the official Meshtastic firmware architecture, you must compile it using their source tree:

1. Clone the Meshtastic firmware repository:
   ```bash
   git clone https://github.com/meshtastic/firmware.git
   cd firmware
   ```
2. Copy this entire `firmware-module/` directory into `firmware/src/modules/meshchain`.
3. In the Meshtastic `src/modules/PluginManager.cpp`, register the `MeshchainModule`:
   ```cpp
   #include "modules/meshchain/MeshchainModule.h"
   // ...
   plugins.push_back(new MeshchainModule());
   ```
4. Build and flash your specific device target using PlatformIO:
   ```bash
   pio run --environment tbeam -t upload
   ```

## Development
To verify the Bincode packer locally without flashing a device, compile and run the test suite:
```bash
g++ test_bincode.cpp -o test_bincode -std=c++11
./test_bincode
```
