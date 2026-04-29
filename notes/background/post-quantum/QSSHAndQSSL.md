# QSSH & QSSL: Post-Quantum Networking in Rust

## Overview

QSSH and QSSL are companion crates from the same author (Paraxiom) that
implement post-quantum replacements for SSH and TLS respectively. They are
**not quantum computing frameworks** — they are classical networking tools
that use post-quantum cryptographic algorithms to resist future quantum
computer attacks.

| Crate | Purpose | Size | Version |
|-------|---------|------|---------|
| `qssh` | Quantum-resistant SSH | 21K SLoC | v0.4.0 |
| `qssl` | Quantum-resistant TLS | 4.5K SLoC | v0.2.0 |

- **Author**: Paraxiom
- **License**: MIT/Apache-2.0
- **Status**: Alpha (no formal security audit for either)
- **Repository**: https://github.com/Paraxiom/qssh (source not published on GitHub)

## Not Quantum Computing

Despite the "quantum" branding, neither crate performs quantum computation
or uses quantum hardware. They implement classical cryptographic algorithms
(Falcon, SPHINCS+, ML-KEM) that are mathematically designed to resist
attacks from future quantum computers — specifically Shor's algorithm
breaking RSA/ECDH and Grover's algorithm weakening symmetric ciphers.

## Why Post-Quantum Algorithms Resist Quantum Attack

Classical cryptography relies on problems that quantum computers solve
efficiently:

| Problem | Used By | Classical Hardness | Quantum Attack |
|---------|---------|-------------------|---------------|
| Integer factoring | RSA | Exponential | Shor's → polynomial |
| Discrete logarithm | ECDH, ECDSA | Exponential | Shor's → polynomial |
| Symmetric key search | AES | 2^128 brute force | Grover's → 2^64 (weakened) |

Post-quantum algorithms use different mathematical problems with no known
efficient quantum algorithm:

| Algorithm | Hard Problem | Why Quantum-Safe |
|-----------|-------------|-----------------|
| ML-KEM (Kyber) | Learning With Errors (LWE) on lattices | No quantum speedup known for lattice problems |
| Falcon | Short Integer Solution (SIS) on lattices | Same — lattice structure resists Shor |
| SPHINCS+ | Hash preimage resistance | Hash-based — Grover only halves security bits (256→128), still safe |

Shor's algorithm exploits hidden algebraic structure (periodicity, group
structure) in factoring and discrete log. Lattice problems and hash
preimages lack this structure, so Shor cannot attack them.

These problems are not *proven* permanently quantum-hard — just that no
quantum attack is known after decades of research. NIST standardized them
(FIPS 203/204/205) after an 8-year public evaluation process.

## QSSH: Quantum-Resistant Secure Shell

A drop-in SSH replacement using NIST-standardized post-quantum algorithms.

### Cryptographic Algorithms

| Component | Classical SSH | QSSH |
|-----------|--------------|------|
| Signatures | RSA, ECDSA | Falcon-512/1024 (FIPS 204), SPHINCS+ (FIPS 205) |
| Key exchange | Diffie-Hellman, ECDH | ML-KEM-768/1024 (FIPS 203) |
| Encryption | AES-256 | AES-256-GCM (unchanged) |

### Security Tiers

| Tier | Name | Description | Requires Hardware? |
|------|------|-------------|-------------------|
| T0 | Classical | RSA/ECDSA (legacy) | No |
| T1 | Post-Quantum | PQC algorithms | No |
| T2 | Hardened PQ | Fixed 768-byte frames | No |
| T3 | Entropy-Enhanced | T2 + QRNG | Yes (QRNG device) |
| T4 | Quantum-Secured | T3 + QKD | Yes (QKD network) |

T0-T2 are software-only. T3+ requires external quantum hardware.

### Key Design: Uniform Frames

All packets are exactly 768 bytes at T2+. An eavesdropper cannot
distinguish handshake messages from data transfer — traditional SSH
leaks message type and size metadata even when encrypted.

### SSH Features

20/21 core features: interactive shell, SFTP, port forwarding
(local/remote/dynamic SOCKS5), ProxyJump, X11 forwarding, connection
multiplexing, agent support, compression, session resumption,
auto-reconnect. Only GSSAPI/Kerberos missing.

## QSSL: Quantum-Safe Secure Layer

A post-quantum TLS replacement emphasizing patent-free cryptography.

### Patent-Free Focus

QSSL's differentiator is avoiding patent-encumbered algorithms:

| Suite | KEM | Patent-Free? |
|-------|-----|-------------|
| Default | SPHINCS+ KEM | ✅ Yes |
| Optional | ML-KEM (Kyber) | ❌ Patent claims |

ML-KEM/Kyber has patent claims that create licensing uncertainty.
SPHINCS+ is hash-based with no known patents. QSSL defaults to
the patent-free option.

### Performance Tradeoff

| Operation | SPHINCS+ KEM | ML-KEM |
|-----------|-------------|--------|
| Encapsulation | ~3 ms | ~60 µs |
| Full handshake | ~8 ms | ~2 ms |

SPHINCS+ is ~50x slower for key exchange but avoids patent risk.

### Features

Implemented: SPHINCS+ and ML-KEM key exchange, Falcon signatures,
AES-GCM and ChaCha20-Poly1305 encryption, fixed-size frames,
post-quantum certificates, session management, HKDF.

In development: certificate chains, 0-RTT, OCSP stapling, WASM.

## Formal Verification

Both crates claim a 3-tier verification pipeline:

| Tier | Tool | QSSH | QSSL |
|------|------|------|------|
| 1 | Kani (AWS) | 30 harnesses | — |
| 2 | Verus (MSR) | 20 proofs | — |
| 3 | Lean 4 + Mathlib | 67 theorems | 100 theorems |

The Lean proofs cover mathematical properties of cryptographic parameters
(prime field instances, NTT compatibility, signature/key size bounds) —
not full protocol correctness. No independent security audit exists.

## Industry Adoption of Post-Quantum Crypto

Post-quantum algorithms are already deployed in production by major platforms:

**Already shipping:**
- **Google Chrome 124+** (April 2024) — ML-KEM hybrid key exchange for all TLS
- **Apple iMessage PQ3** (March 2024) — ML-KEM for end-to-end encryption
- **Signal PQXDH** (September 2023) — ML-KEM for key agreement
- **Cloudflare** (October 2024) — ML-KEM hybrid TLS for all sites
- **OpenSSH 9.0+** (2022) — sntrup761 hybrid key exchange available

**Mainstream library support:**
- **OpenSSL 3.5.0** (April 2025) — native ML-KEM, ML-DSA, SLH-DSA
- **OpenSSL 4.0.0** (April 2026) — enhanced PQ + Encrypted Client Hello

With OpenSSL 3.5 and TLS 1.3, hybrid key exchange (`x25519_mlkem768`)
is enabled by default if both client and server support it — no code
changes needed. This means the mainstream TLS stack already provides
the same post-quantum key exchange that QSSL implements, reducing the
practical need for custom PQ TLS implementations.

**Federal mandates:**
- NIST finalized FIPS 203/204/205 in August 2024
- US federal agencies must migrate to PQC by 2035
- "Harvest now, decrypt later" threat drives urgency — adversaries
  recording encrypted traffic today for future quantum decryption

## Relevance to qcfront

These crates are tangentially related:

- **Post-quantum motivation**: The algorithms QSSH/QSSL defend against
  (Shor breaking RSA, Grover weakening AES) are the same algorithms we
  implement in `algos::shor` and `algos::grover`. Our implementations
  demonstrate *why* post-quantum crypto is needed.
- **No overlap**: Different problem domain. We build quantum algorithms;
  QSSH/QSSL build quantum-resistant networking.
- **Same ecosystem**: Both are pure Rust, showing the breadth of
  quantum-adjacent work happening in the Rust ecosystem.

## Concerns

- **GitHub repos have no source code** — only LICENSE and README published.
  Crates are on crates.io but source isn't publicly reviewable.
- **No security audit** — critical for security-focused tools.
- **Alpha status** — authors recommend against production deployment.
- **QSSH is 4.1 MB** on crates.io with bundled ELF binaries in tests —
  unusual for a library crate.

## Sources

- QSSH: https://lib.rs/crates/qssh / https://docs.rs/qssh
- QSSL: https://lib.rs/crates/qssl / https://docs.rs/qssl
- GitHub: https://github.com/Paraxiom/qssh (source not published)
