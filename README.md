# BIP-340 verification cost inside a RISC Zero guest

Measures what it costs to verify M BIP-340 (secp256k1 Schnorr, x-only) signatures inside a
RISC Zero zkVM guest, with real proving. Built for Logos RFP-005 (Multisig and
Coordination), where the cost of verifying M approvals at execution is the largest
unpriced item (P.2). Results feed §7 of Catalyst Labs' RFP-005 proposal, Tessera.

This is the in-guest verification cost in isolation. It is not a whole LEZ
privacy-preserving transaction; that is measured in the grant's first milestone.

## Results (Apple M4, 10 cores, 24 GB, CPU-only proving, `RISC0_DEV_MODE=0`)

| M | user cycles, accelerated `k256` | share of 10M gas | user cycles, stock `k256` | real proof | peak RSS | receipt |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 306,497 | 3.1% | 5,428,119 | 48.4 s | 3.6 GB | 268 kB |
| 3 | 897,786 | 9.0% | 16,268,589 | – | – | – |
| 5 | 1,493,886 | 14.9% | 27,109,059 | 206.1 s | 7.7 GB | 563 kB |
| 7 | 2,086,332 | 20.9% | 37,949,529 | – | – | – |
| 10 | 2,980,826 | 29.8% | 54,210,234 | 338.6 s | 7.9 GB | 1,054 kB |
| 16 | 4,754,287 | 47.5% | 86,731,644 | 586.7 s | 8.7 GB | 1,407 kB |
| 32 | 9,512,524 | 95.1% | 173,455,404 | – | – | – |

Succinct receipt, M = 5: 262.7 s, 8.5 GB, 223 kB.

- About 298k cycles per approval with the accelerated crate, linear in M. M = 32 still
  fits under LEZ's 10M execution-gas cap per public transaction.
- Stock `k256` costs 5.4M cycles per verification, so M = 2 does not fit at all.
- Receipts are composite (one segment per ~1M cycles) unless stated.
- Stock `k256`, M = 1, real proof: 576.5 s, 7.9 GB, 1.67 MB composite receipt (six segments),
  about 12 times slower than the accelerated crate's 48.4 s for the same check.

Full method, exact versions and caveats: [RESULTS.md](RESULTS.md). Raw logs: [logs/](logs/)
(local filesystem paths in the logs are replaced with `<repo>` and `<home>`; nothing else is edited).

## Layout

- `methods/guest/` verifies with RISC Zero's accelerated `k256` fork (patched `k256`,
  `crypto-bigint`, `sha2`).
- `methods/guest_stock/` is the same source with stock crates.io `k256`, for comparison.
- `host/` generates keys and signatures, runs the executor and the prover, prints results.

## Reproduce

```bash
curl -L https://risczero.com/install | bash
rzup install cargo-risczero 3.0.5
rzup install r0vm 3.0.5
rzup install rust
cargo build --release

# cycle counts (executor only)
RISC0_DEV_MODE=0 ./target/release/host exec accel 1 3 5 7 10 16 32
RISC0_DEV_MODE=0 ./target/release/host exec stock 1 3 5 7 10 16 32

# real proving
RISC0_DEV_MODE=0 RISC0_INFO=1 /usr/bin/time -l ./target/release/host prove accel 5
RISC0_DEV_MODE=0 RISC0_INFO=1 /usr/bin/time -l ./target/release/host prove-succinct accel 5
```

`run_proofs.sh` runs the proving set used for the table and writes to `logs/`.

## Licence

MIT OR Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
