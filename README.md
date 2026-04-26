# OpenPeer

P2P end-to-end encrypted file transfer using Noise Protocol and STUN/TURN NAT traversal.

## Features

- Direct peer-to-peer connections — no cloud storage, no data passing through central servers
- End-to-end encryption via Noise Protocol (XX pattern, ChaCha20-Poly1305, BLAKE2s)
- NAT traversal via STUN + mandatory TURN relay fallback
- Resumeable file transfers with SHA-256 integrity verification
- Streaming chunked transfer (64 KiB default), progress display

## Build

```bash
cargo build --release
```

Requires Rust 1.85+ (stable).

## Test

```bash
cargo test --all-features
cargo llvm-cov --all-features --workspace  # requires cargo-llvm-cov
```

## Run

### Signaling server

```bash
cargo run --bin opserver -- --bind 0.0.0.0:38901 --external-addr <your-public-ip>:38901
```

### Client

```bash
cargo run --bin opclient -- --help
```

## Architecture

| Module | Responsibility |
|---|---|
| `protocol/` | Wire format messages + binary codec (no IO) |
| `crypto/` | Noise Protocol session state machines (no IO) |
| `signaling/` | Server-side registry and peer matchmaking |
| `p2p/` | NAT punching, Noise handshake, encrypted stream |
| `transfer/` | Chunked file protocol on encrypted stream |
| `cli/` | User interaction only; no business logic |

## License

MIT OR Apache-2.0
