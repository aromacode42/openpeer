# CLAUDE.md — OpenPeer

Operating contract for Claude Code in this repository. **Read this file in full before any task.**

---

## 1. Project Overview

**OpenPeer** is a Rust P2P file-transfer system featuring:

- Direct client-to-client connections (P2P)
- End-to-end encryption (Noise Protocol)
- A lightweight signaling server used **only** for coordination — never for data
- NAT traversal (STUN/TURN)
- Single-file transfer, resume, SHA256 integrity, progress display

**Core principle:** the central server handles signaling only. Data traffic must never pass through it (except via TURN relay, which stays end-to-end encrypted).

---

## 2. Locked Technical Decisions (do not change without user approval)

| Area | Choice | Notes |
|---|---|---|
| Language | Rust, edition 2021 | stable toolchain pinned via `rust-toolchain.toml` |
| Async runtime | `tokio` (full features) | |
| Encryption | **Noise Protocol** via `snow` | pattern: `Noise_XX_25519_ChaChaPoly_BLAKE2s` |
| Signaling transport | TCP | |
| Signaling encoding | **Custom binary protocol** | length-prefixed frames, see §5 |
| P2P data transport | TCP for MVP, UDP reserved | |
| NAT traversal | **Full STUN + TURN** | TURN is mandatory fallback |
| Project layout | **Single crate, multiple modules** | multiple binaries under `src/bin/` |
| Server deployment | China-mainland VPS | NAT environment is hostile, see §11 |
| Integrity | SHA256 (whole file + per chunk) | `sha2` crate |
| Serialization | hand-rolled for wire protocol; `bincode` for internal state | |
| Coverage tool | **`cargo-llvm-cov`** | target ≥95% line coverage, aim for ~100% |
| Repo hosting | **GitHub, public** | see §15 |
| CI/CD | GitHub Actions | see §16 |
| Release targets | Linux x86_64, macOS (x86_64 + aarch64), Windows x86_64 | static builds where possible |

---

## 3. Repository Layout (must follow)

```text
openpeer/
├── Cargo.toml
├── Cargo.lock                 # committed (binary crate)
├── CLAUDE.md
├── README.md
├── LICENSE                    # ask user: MIT, Apache-2.0, or dual
├── CHANGELOG.md
├── rust-toolchain.toml
├── .gitignore
├── .github/
│   ├── workflows/
│   │   ├── ci.yml             # fmt + clippy + test + coverage
│   │   └── release.yml        # tag-triggered multi-OS build + GitHub Release
│   ├── dependabot.yml
│   ├── ISSUE_TEMPLATE/
│   └── PULL_REQUEST_TEMPLATE.md
├── src/
│   ├── lib.rs                 # public API surface
│   ├── bin/
│   │   ├── opserver.rs        # signaling/STUN/TURN server entry
│   │   └── opclient.rs        # client CLI entry
│   ├── protocol/              # signaling wire protocol
│   │   ├── mod.rs
│   │   ├── codec.rs
│   │   └── messages.rs
│   ├── crypto/                # E2EE
│   │   ├── mod.rs
│   │   ├── noise.rs
│   │   └── hash.rs
│   ├── signaling/             # signaling server logic
│   │   ├── mod.rs
│   │   ├── server.rs
│   │   ├── session.rs
│   │   └── stun_turn.rs
│   ├── p2p/                   # peer connection management
│   │   ├── mod.rs
│   │   ├── connector.rs
│   │   ├── transport.rs
│   │   └── nat.rs
│   ├── transfer/              # file transfer logic
│   │   ├── mod.rs
│   │   ├── sender.rs
│   │   ├── receiver.rs
│   │   ├── chunk.rs
│   │   ├── resume.rs
│   │   └── progress.rs
│   ├── cli/                   # CLI parsing + UI
│   │   ├── mod.rs
│   │   └── ui.rs
│   ├── config.rs
│   └── error.rs
├── tests/                     # integration tests
│   ├── integration_signaling.rs
│   ├── integration_transfer.rs
│   ├── integration_resume.rs
│   ├── integration_nat.rs
│   └── common/mod.rs
└── benches/                   # criterion benches (optional)
```

Do not introduce new top-level directories without user approval.

---

## 4. Module Boundaries (strict)

- **`protocol/`** — wire message types and codec only. No runtime state, no IO.
- **`crypto/`** — primitives and session state machines. No IO.
- **`signaling/`** — server side: registry, matchmaking, relay candidate exchange. Never touches file data.
- **`p2p/`** — turns "two endpoints" into "one bidirectional encrypted stream". Includes NAT punching and Noise handshake. Format-agnostic.
- **`transfer/`** — runs chunked file protocol on top of the encrypted stream. Owns progress, resume, integrity.
- **`cli/`** — user interaction only. No business logic; calls into the lib API.

Dependency direction is bottom-up only. **No reverse dependencies.**

---

## 5. Signaling Wire Protocol (MVP)

Frame format:

```text
+--------+--------+----------+----------------+
| MAGIC  |  VER   |  MSG_TY  |  LEN (u32 BE)  |
| 2 byte | 1 byte |  1 byte  |     4 byte     |
+--------+--------+----------+----------------+
|              PAYLOAD (LEN bytes)            |
+---------------------------------------------+
```

- `MAGIC` = `0x4F50` ("OP")
- `VER` = `0x01`
- All multi-byte integers: **big-endian**
- Strings inside payload: `u16 BE length + UTF-8 bytes`
- `SocketAddr`: `u8 family (4=v4, 6=v6) + addr bytes + u16 port`

Message types (MVP set, extensible):

| Code | Name | Direction | Payload |
|---|---|---|---|
| `0x01` | `Register` | C→S | `peer_id: String, pubkey: [u8;32]` |
| `0x02` | `RegisterAck` | S→C | `assigned_id: String, public_addr: SocketAddr` |
| `0x03` | `ConnectRequest` | C→S | `target_peer_id: String` |
| `0x04` | `ConnectOffer` | S→C | `peer_id, peer_addr, peer_pubkey, session_id` |
| `0x05` | `ConnectAccept` | C→S | `session_id` |
| `0x06` | `RelayCandidate` | S↔C | `session_id, candidate (STUN/TURN)` |
| `0x07` | `Heartbeat` | C→S | empty |
| `0x08` | `Bye` | C→S | empty |
| `0xFF` | `Error` | S→C | `code: u16, msg: String` |

The chunked file protocol used over the encrypted P2P stream is defined separately in `transfer/` and is **not** part of the signaling protocol.

---

## 6. Cryptography Requirements (non-negotiable)

- Each client generates a long-term X25519 keypair on first launch, persisted at `~/.openpeer/identity.key` with file mode `0600` on Unix.
- Per-session Noise `XX` handshake yields paired send/recv symmetric keys.
- File data: every chunk is independently AEAD-encrypted (ChaCha20-Poly1305); nonce derived from `chunk_index`.
- The signaling server **never** sees file keys. It only forwards public keys.
- Logs must never contain keys, plaintext chunks, or file contents. IPs at `info` level should be partially masked.

---

## 7. Error Handling and Logging

- Single error type: `crate::error::Error` using `thiserror`.
- Public APIs return `Result<T, Error>`.
- **No `unwrap()` / `expect()` in non-test code.** Startup-only fatals may use `expect("human-readable reason")`.
- Logging: `tracing` + `tracing-subscriber`.
  - `error!` — unrecoverable
  - `warn!` — recoverable but notable
  - `info!` — connection lifecycle, transfer start/end
  - `debug!` — handshake details, chunk progress
  - `trace!` — byte-level (off by default)
- No `println!` in production paths.

---

## 8. Coding Standards

- `cargo fmt` clean.
- `cargo clippy --all-targets --all-features -- -D warnings` clean.
- Public items have `///` doc comments.
- Async fns are not suffixed with `_async`.
- Network buffers use `bytes::Bytes` / `BytesMut`.
- Every IO call is wrapped in `tokio::time::timeout` (signaling default 10s, handshake 15s).
- Configuration via `config.rs`: TOML file < env vars < CLI flags (later overrides earlier).
- `unsafe` is forbidden by default; any usage requires PR justification.

---

## 9. Test-Driven Development (mandatory)

OpenPeer is developed strictly TDD. **Production code is never written before its failing test exists.**

### 9.1 The TDD cycle (every feature, no exceptions)

1. **Red** — write a failing test that captures the desired behavior. Run it; confirm it fails for the right reason.
2. **Green** — write the minimum production code needed to pass the test. Nothing more.
3. **Refactor** — clean up while tests stay green.
4. Commit at the end of each cycle (see §15.3).

If you find yourself writing production code with no failing test pointing at it, stop and write the test first.

### 9.2 Test layers

| Layer | Location | Purpose |
|---|---|---|
| Unit | `#[cfg(test)] mod tests` inside each source file | pure functions, codecs, state machines |
| Integration | `tests/*.rs` | server + client end-to-end on `127.0.0.1` |
| Property | `proptest` inside unit modules | codec round-trips, chunking invariants |
| Doc tests | `///` examples on public APIs | every exported function with non-trivial behavior |

### 9.3 Required test scenarios (minimum)

- Wire-protocol encode→decode round-trip for **every** message type, including malformed/truncated frames.
- Noise handshake completes on a duplex in-memory transport; tampered ciphertext is rejected.
- Two clients register with the signaling server and are matched.
- End-to-end file transfer over loopback for sizes: empty, 1 byte, 1 KiB, 1 MiB, 100 MiB.
- Mid-transfer disconnect → reconnect → resume completes; final SHA256 matches.
- Corrupted chunk → receiver rejects and recovers.
- TURN relay path: same e2e test forced through relay.

### 9.4 Coverage targets

- Tooling: `cargo-llvm-cov`.
- Required: **≥95% line coverage** on the workspace, aim for ~100%.
- CI fails the build if coverage drops below the threshold (see §16.1).
- Coverage report uploaded to Codecov on every push to `main` and on PRs.
- Acceptable to exclude: `src/bin/*` thin entry points, generated code. Document any exclusion in workflow comments or `.cargo/llvm-cov.toml`.

### 9.5 Test hygiene

- Tests must be deterministic. No real network beyond loopback, no real clock dependencies (use `tokio::time::pause` or inject clocks).
- Bind to ephemeral ports: `TcpListener::bind("127.0.0.1:0")`.
- No `sleep`-based synchronization; use channels, barriers, or polling with a timeout.
- Each test cleans up its temp files (`tempfile` crate).
- Slow tests gated behind `#[ignore]` or a feature flag.

---

## 10. Performance and Observability

- Default chunk size: **64 KiB** (configurable).
- Streaming IO only; **never** load whole files into memory.
- Progress events flow over `tokio::sync::mpsc` to the CLI, rendered with `indicatif`.
- Hot paths instrumented with `tracing::Span` for future OpenTelemetry export.
- Optional `criterion` benches under `benches/` for codec and chunking.

---

## 11. China-Mainland Network Notes

- ISP NAT is frequently symmetric; pure STUN often fails. **TURN fallback is required.**
- IPv6 is widely available; prefer it via happy-eyeballs.
- Avoid commonly throttled or filtered ports. Defaults: signaling `38901`, TURN `38902`.
- Keep signaling packets small to avoid traffic-anomaly triggers.
- Server binary must support `--bind` and `--external-addr` for deployment behind cloud LBs.

---

## 12. Claude Code Working Rules

For every task:

1. **Read before write.** `view` any file before modifying it.
2. **Small steps.** One subtask at a time; run `cargo check && cargo test` before moving on.
3. **TDD always** (§9). No production code without a failing test first.
4. **Don't reinvent.** Use mature crates for crypto, STUN, parsing. If selection is unclear, **stop and ask**.
5. **Stay in scope.** Do not silently expand the task (no surprise GUI, no surprise web).
6. **Respect module boundaries** (§4). Flag violations before fixing them.
7. **Never lower security.** Don't disable encryption, skip checks, or loosen permissions to make a test pass.
8. **Ask on ambiguity.** Present options; do not guess.
9. **Sync progress.** After each milestone summarize what changed and what's next.
10. **Rust explanations:** do not draw analogies to Java. Use tables and lists; keep code snippets minimal.
11. **Commit discipline** (§15.3). Every green TDD cycle ends with a commit.

---

## 13. Milestones (suggested)

- **M0** Skeleton: Cargo init, module stubs, CI (fmt/clippy/test/coverage), error type, logging, GitHub repo, branch protection, release workflow.
- **M1** Wire protocol: codec + server registration/matchmaking. No crypto, no files.
- **M2** Noise handshake: XX over P2P TCP, bidirectional echo.
- **M3** File transfer MVP: chunking + AEAD + SHA256 + progress bar.
- **M4** Resume: receiver persists per-chunk bitmap; reconnect resumes.
- **M5** STUN hole-punching: NAT-type detection, prefer direct.
- **M6** TURN relay: automatic fallback when punching fails.
- **M7** CLI polish, docs, first stable release.

Each milestone ends with: all tests green, coverage ≥95%, tag pushed, GitHub Release published with binaries.

---

## 14. Forbidden

- ❌ Routing file data through the signaling server (TURN relay excepted, and only encrypted)
- ❌ Storing user files or plaintext keys server-side
- ❌ A "no-encryption" toggle of any kind
- ❌ `unsafe` blocks without justification
- ❌ GPL-family transitive dependencies
- ❌ `println!` in production code
- ❌ Writing production code before a failing test exists
- ❌ Merging to `main` with red CI or coverage below threshold

---

## 15. Git and GitHub

### 15.1 Repository

- Hosted on **GitHub, public**.
- Repo name: `openpeer` (final name confirmed by user before `git remote add`).
- Default branch: `main`.
- License file committed at root (ask user: MIT, Apache-2.0, or dual).
- `.gitignore` covers: `/target`, `*.key`, `~/.openpeer/`, IDE files. `Cargo.lock` is **kept** (binary crate).

### 15.2 Branching

- `main` is always green and releasable.
- Feature work on `feat/<short-name>`, fixes on `fix/<short-name>`.
- PRs only into `main`. Required: CI green, coverage ≥95%, ≥1 self-review pass.
- Branch protection on `main`: require PR, require status checks (`ci` + `coverage`), require linear history.

### 15.3 Commits

- Conventional Commits format: `feat:`, `fix:`, `test:`, `refactor:`, `docs:`, `chore:`, `ci:`.
- One TDD cycle = one commit (or a small commit pair: `test:` then `feat:`).
- Commit message body explains *why*, not *what*.
- No commits with failing tests on `main`.

### 15.4 Tags and releases

- Semantic versioning: `vMAJOR.MINOR.PATCH`.
- Pre-1.0 milestones tagged as `v0.M.0` (M0 → `v0.0.0` skeleton, M1 → `v0.1.0`, etc.).
- Tag push triggers the release workflow (§16.2).
- `CHANGELOG.md` maintained at root, Keep-a-Changelog style.

---

## 16. CI/CD (GitHub Actions)

Two workflows are mandatory and must exist from M0.

### 16.1 `.github/workflows/ci.yml` — every push and PR

Jobs:

1. **lint** — `cargo fmt --check` + `cargo clippy --all-targets --all-features -- -D warnings`.
2. **test** — matrix over `{ubuntu-latest, macos-latest, windows-latest}` × stable toolchain. Runs `cargo test --all-features`.
3. **coverage** — Linux only. Installs `cargo-llvm-cov`, runs `cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info`. Uploads to Codecov. **Fails the job if line coverage < 95%.**
4. **docs** — `cargo doc --no-deps` must succeed with `-D warnings` on rustdoc.

Caching: `Swatinem/rust-cache@v2` on every job.

### 16.2 `.github/workflows/release.yml` — on tag `v*`

Jobs:

1. **build** matrix:
   - `x86_64-unknown-linux-gnu` on `ubuntu-latest`
   - `x86_64-apple-darwin` on `macos-13`
   - `aarch64-apple-darwin` on `macos-latest`
   - `x86_64-pc-windows-msvc` on `windows-latest`
   Each builds `opserver` and `opclient` in release mode, strips symbols, packages as `.tar.gz` (Unix) or `.zip` (Windows) named `openpeer-<version>-<target>.{tar.gz,zip}`, generates SHA256 checksum file.
2. **release** — depends on build. Uses `softprops/action-gh-release@v2` to create the GitHub Release for the tag. Attaches all archives and `SHA256SUMS`. Body is generated from `CHANGELOG.md` for the matching version.

Permissions: `contents: write` only on the release job. No long-lived secrets; rely on `GITHUB_TOKEN`.

### 16.3 Dependabot

Enable `.github/dependabot.yml` for `cargo` and `github-actions` ecosystems, weekly cadence.

---

## 17. Definition of Done (every PR)

- [ ] Failing test was written first; commit history shows red→green
- [ ] All tests pass on Linux, macOS, Windows
- [ ] `cargo fmt` and `cargo clippy -D warnings` clean
- [ ] Line coverage ≥95% (workspace)
- [ ] Public APIs documented; `cargo doc` clean
- [ ] No new `unsafe`, no new `unwrap` in production paths
- [ ] CHANGELOG updated under `[Unreleased]`
- [ ] No regressions in existing integration tests
- [ ] Module boundaries (§4) respected
- [ ] Logs reviewed for sensitive-data leaks

---

*This document is the contract between OpenPeer and Claude Code. Any change requires user confirmation.*
