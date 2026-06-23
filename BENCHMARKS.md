# Benchmarks — Microscope Memory v0.8.1

## System

- **Binary size:** 2.2 MB (release, stripped)
- **Memory index:** 1010 KB (28679 blocks, 9 depths)
- **Block size:** 256 bytes data + 32 byte header

## Query Performance (10,000 queries per zoom level)

| Zoom | Blocks | Avg Query Time |
|------|--------|---------------|
| D0   | 1      | 63.2 µs |
| D1   | 5      | 57.6 µs |
| D2   | 27     | 63.8 µs |
| D3   | 129    | 69.1 µs |
| D4   | 336    | 80.2 µs |
| D5   | 1632   | 127.5 µs |
| D6   | 4183   | 166.3 µs |
| D7   | 11104  | 191.4 µs |
| D8   | 11262  | 189.3 µs |

**Overall average:** 112 µs/query

## Soft 4D Zoom

| Mode | Time |
|------|------|
| 4D soft (all 28679 blocks) | 380 µs/query |

## Comparison: Microscope vs Vector Databases

| System | Query Type | Avg Latency | Index Size | Notes |
|--------|-----------|-------------|------------|-------|
| **Microscope Memory** | Exact spatial recall | **112 µs** | 1010 KB | 28679 blocks, 9 depths |
| FAISS (flat IP) | Approximate k-NN | ~1-5 ms | ~10-50 MB | Industry standard |
| Pinecone | Approximate vector search | ~5-20 ms | hosted | Managed service |
| ChromaDB | Approximate vector search | ~5-50 ms | ~10-100 MB | Local, disk-based |
| Qdrant | Approximate vector search | ~4-15 ms | ~10-50 MB | Local or hosted |
| Weaviate | Approximate vector search | ~5-30 ms | hosted | Managed service |

**Key difference:** Microscope uses zoom-based hierarchical spatial indexing (D0-D8), not approximate vector search.  
It trades semantic fuzziness for deterministic, sub-millisecond exact recall.

## Integrity

- **CRC16 verified:** 28679 blocks OK, 0 errors
- **Merkle Tree:** verified

## Storage

| Metric | Value |
|--------|-------|
| Total memory index | 1010 KB |
| Headers | 896 KB |
| Data | 114 KB |
| Viewport | 256 chars/block |
| Cache | L3 |

## Tests

- **Unit tests:** 253 passed, 0 failed
- **Build:** release mode, LTO thin, panic=abort

## Build

```
cargo build --release
```