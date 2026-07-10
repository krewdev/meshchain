# Truth model

**Confirmed balance** = account balance after the latest **final** block.

A block is final when ≥ `ceil(2N/3)` validators have ACK'd it (N = validator set size).

- Pending txs (mempool / unfinalized) must not be treated as spendable income.  
- Wallet spendable = finalized balance − pending outgoing.  
- Mesh is the source of truth for MESH; Solana vault only moves value **in/out** via Mint/Burn.  
