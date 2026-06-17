# Benchmarks — Microscope Memory v0.8.0

## System

- **Binary size:** 5.4 MB (release, stripped)
- **Memory index:** 722 KB (20323 blocks, 9 depths)
- **Block size:** 256 bytes data + 32 byte header

## Query Performance (10,000 queries per zoom level)

| Zoom | Blocks | Avg Query Time |
|------|--------|---------------|
| D0   | 1      | 99.1 µs |
| D1   | 5      | 99.3 µs |
| D2   | 21     | 90.9 µs |
| D3   | 94     | 78.4 µs |
| D4   | 225    | 71.2 µs |
| D5   | 1229   | 74.4 µs |
| D6   | 2886   | 78.2 µs |
| D7   | 7914   | 98.8 µs |
| D8   | 7948   | 95.2 µs |

**Overall average:** 87.2 µs/query

## Soft 4D Zoom

| Mode | Time |
|------|------|
| 4D soft (all 20323 blocks) | 249 µs/query |

## Integrity

- **CRC16 verified:** 20323 blocks OK, 0 errors
- **Merkle Tree:** verified

## Storage

| Metric | Value |
|--------|-------|
| Total memory index | 722 KB |
| Headers | 635 KB |
| Data | 87 KB |
| Viewport | 256 chars/block |
| Cache | L3 |

## Tests

- **Unit tests:** 238 passed, 0 failed
- **Build:** release mode, LTO thin, panic=abort

## Build

```
cargo build --release
```
