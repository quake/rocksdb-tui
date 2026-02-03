# CKB RocksDB Configuration

This directory contains the configuration files for browsing CKB (Nervos Network) RocksDB databases with `rocksdb-tui`.

## Files

- `config.toml` - Parser configuration for all 19 CKB column families
- `schemas/blockchain.mol` - Core blockchain Molecule types
- `schemas/extensions.mol` - Extended storage and network Molecule types

## Usage

```bash
# Browse CKB mainnet database
rocksdb-tui --db ~/.ckb/data/db --config examples/ckb/config.toml

# Browse CKB testnet database
rocksdb-tui --db ~/.ckb/testnet/data/db --config examples/ckb/config.toml
```

## Column Families

| Column | Name | Description | Value Type |
|--------|------|-------------|------------|
| 0 | COLUMN_INDEX | Chain index (number → hash) | Byte32 |
| 1 | COLUMN_BLOCK_HEADER | Block headers | HeaderView |
| 2 | COLUMN_BLOCK_BODY | Block bodies (transactions) | TransactionVec |
| 3 | COLUMN_BLOCK_UNCLE | Uncle blocks | UncleBlockVecView |
| 4 | COLUMN_META | Meta data | Raw bytes |
| 5 | COLUMN_TRANSACTION_INFO | Transaction info | TransactionInfo |
| 6 | COLUMN_BLOCK_EXT | Block extension info | BlockExt |
| 7 | COLUMN_BLOCK_PROPOSAL_IDS | Proposal IDs | ProposalShortIdVec |
| 8 | COLUMN_BLOCK_EPOCH | Block epoch mapping | Raw bytes |
| 9 | COLUMN_EPOCH | Epoch data | EpochExt |
| 10 | COLUMN_CELL | Live cells | CellEntry |
| 11 | COLUMN_UNCLES | Uncles tracking | Raw bytes |
| 12 | COLUMN_CELL_DATA | Cell data | CellDataEntry |
| 13 | COLUMN_NUMBER_HASH | Number-hash pairs | NumberHash |
| 14 | COLUMN_CELL_DATA_HASH | Cell data hashes | Raw bytes |
| 15 | COLUMN_BLOCK_EXTENSION | Block extension | Raw bytes |
| 16 | COLUMN_CHAIN_ROOT_MMR | Chain root MMR | Raw bytes |
| 17 | COLUMN_BLOCK_FILTER | Block filter data | Raw bytes |
| 18 | COLUMN_BLOCK_FILTER_HASH | Block filter hash | Raw bytes |

## Notes

- CKB uses numeric column family names ("0", "1", etc.)
- Most storage types are defined in `extensions.mol`
- Some columns store raw bytes without Molecule encoding
