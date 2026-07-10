---
description: Run vault deposit → tMESH mint e2e on testnet
---

Guide or run MeshChain e2e flow:

1. Solana devnet deposit into program `CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx`
2. Create mesh wallet (`mesh new-wallet`) — note mesh name
3. `meshchain-node mint-for-deposit` with net lamports + external ref
4. Verify with scanner or `mesh balance`

Scripts: `programs-mesh-bridge/scripts/e2e_vault_to_mesh.ts`, `mint-for-deposit` CLI.
Testnet only. Check `solana balance` and disk space before large builds.
