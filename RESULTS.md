# BIP-340 (secp256k1 Schnorr, x-only) batch verification inside RISC Zero zkVM — benchmark

Date: 2026-10-23. All numbers below were measured on the machine described here; nothing is extrapolated unless explicitly labelled as such.

## Hardware / OS

| Item | Value |
|---|---|
| CPU | Apple M4 (10 cores: `hw.ncpu` = 10) |
| RAM | 24 GB (`hw.memsize` = 25769803776) |
| OS | macOS 26.6.2 (Darwin 25.6.0, build 25G83) |
| GPU use | **none** — see "Prover backend" below (CPU-only proving) |

## Toolchain (exact versions)

| Component | Version | How installed |
|---|---|---|
| rzup | 0.5.0 | `curl -L https://risczero.com/install \| bash` |
| cargo-risczero | 3.0.5 | `rzup install cargo-risczero 3.0.5` |
| r0vm (external prover/executor binary) | 3.0.5 (`risc0-r0vm 3.0.5`) | `rzup install r0vm 3.0.5` (bundled with cargo-risczero extension) |
| risc0 guest Rust toolchain | rust 1.97.0 (`rzup install rust`) | `~/.risc0/toolchains/v1.97.0-rust-aarch64-apple-darwin` |
| host Rust | rustc 1.99.1 (58cf8f9c3 2026-10-13), cargo 1.98.1 | pre-existing stable |
| `risc0-zkvm` crate (host + guest) | 3.0.6 (resolved from `^3.0.5`) | crates.io |
| `risc0-build` | 3.0.6 | crates.io |
| `risc0-circuit-rv32im` | 4.0.5 | crates.io (pulled by risc0-zkvm) |
| `risc0-zkp` | 3.0.5 | crates.io |
| `risc0-zkvm-platform` | 2.2.3 | crates.io |
| `risc0-bigint2` (guest, accel variant only) | 1.4.14 | crates.io (pulled by forked k256) |

## Crates used for secp256k1 / Schnorr

Two guest programs with identical source (`methods/guest/src/main.rs` == `methods/guest_stock/src/main.rs`), differing only in `Cargo.toml`:

**`verify_sigs` (accelerated, the primary measurement)** — `methods/guest/Cargo.toml`:

```toml
k256 = { version = "=0.13.4", default-features = false, features = ["schnorr", "std"] }

[patch.crates-io]
k256          = { git = "https://github.com/risc0/RustCrypto-elliptic-curves", tag = "k256/v0.13.4-risczero.1" }   # commit 7d4a8d6477e258ce4169ee4669cf92aee0582c39
crypto-bigint = { git = "https://github.com/risc0/RustCrypto-crypto-bigint",   tag = "v0.5.5-risczero.0" }         # commit 3ab63a6f1048833f7047d5a50532e4a4cc789384
sha2          = { git = "https://github.com/risc0/RustCrypto-hashes",          tag = "sha2-v0.10.8-risczero.0" }   # commit 244dc3b08788f7a4ccce14c66896ae3b4f24c166
```

The `[patch.crates-io]` pattern (k256 + crypto-bigint + sha2) is the one used by the official `risc0/examples/ecdsa/k256` on the `release-3.0` branch (that example pins `k256/v0.13.3-risczero.1` + `crypto-bigint v0.5.2-risczero.0` + `sha2-v0.10.6-risczero.0`; we used the newest tags of each, which built cleanly). Using the forked `k256` *without* the `crypto-bigint` patch does not compile (`unresolved import elliptic_curve::bigint::risc0`). The `RISC0_INFO=1` prover log confirms the precompiles are in effect: for M=1 the session shows `1854 BigInt calls, 138514 cycles (26.42%)` and `4 Sha2 calls, 296 cycles`.

**`verify_sigs_stock` (comparison)** — `methods/guest_stock/Cargo.toml`: stock crates.io `k256 = "=0.13.4"` with `features = ["schnorr","std"]`, no patches (pure-software field arithmetic; crates.io `crypto-bigint 0.5.5`, `sha2 0.10.9`).

Host signing uses crates.io `k256 0.13.4` (`k256::schnorr::SigningKey::random(&mut OsRng)`, `sign_prehash`), `rand_core 0.6` (getrandom), `bincode 1.3` for receipt serialization.

Guest profile (both): `opt-level = 3` (workspace), `codegen-units = 1`, `lto = "thin"`.

## Guest program

```rust
use k256::schnorr::{signature::hazmat::PrehashVerifier, Signature, VerifyingKey};
use risc0_zkvm::guest::env;

fn main() {
    let msg: [u8; 32] = env::read();
    let pubkeys: Vec<[u8; 32]> = env::read();   // x-only pubkeys
    let sigs: Vec<u8> = env::read();            // 64 * M bytes
    let m = pubkeys.len();
    assert_eq!(sigs.len(), 64 * m);
    for i in 0..m {
        let vk = VerifyingKey::from_bytes(&pubkeys[i]).expect("invalid x-only pubkey");
        let sig = Signature::try_from(&sigs[i * 64..(i + 1) * 64]).expect("invalid signature encoding");
        vk.verify_prehash(&msg, &sig).expect("BIP-340 signature verification failed"); // panics -> no proof
    }
    env::commit(&(m as u32));
}
```

All M signatures are over the same 32-byte message (`[0x42; 32]`), each by a fresh random key. Verification is M independent `verify_prehash` calls (no batch-verification trick). The journal commits M.

## Exact commands

```bash
# toolchain
curl -L https://risczero.com/install | bash
~/.risc0/bin/rzup install cargo-risczero 3.0.5
~/.risc0/bin/rzup install r0vm 3.0.5
~/.risc0/bin/rzup install rust              # -> 1.97.0
cargo risczero --version; r0vm --version    # 3.0.5 / 3.0.5

# project
cargo risczero new bip340bench --guest-name verify_sigs   # then edited as described above
cd bip340bench && cargo build --release

# executor only (cycle counts) — uses the external r0vm executor via default_executor()
RISC0_DEV_MODE=0 ./target/release/host exec accel 1 3 5 7 10 16 32
RISC0_DEV_MODE=0 ./target/release/host exec stock 1 3 5 7 10 16 32

# real proving (dev mode explicitly OFF), composite STARK receipt via default_prover()
RISC0_DEV_MODE=0 RISC0_INFO=1 RUST_LOG=info /usr/bin/time -l ./target/release/host prove accel <M>
# succinct receipt
RISC0_DEV_MODE=0 RISC0_INFO=1 RUST_LOG=info /usr/bin/time -l ./target/release/host prove-succinct accel 5
```

The host binary and all sources are in this folder; raw logs are in `logs/` (`10_exec_accel.log`, `11_exec_stock.log`, `20_prove_accel_M*.log`, `21_prove_succinct_accel_M5.log`, `run_proofs.out`, build/install logs `00`-`04`).

## Prover backend (important caveat)

- `default_prover()` / `default_executor()` in `risc0-zkvm 3.0.6` without the `prove` feature use the **external `r0vm 3.0.5` binary over IPC** (`ExternalProver("ipc")`). The host was built this way (default features), so all proving was done by the rzup-installed `r0vm`.
- The r0vm binary is linked against `Metal.framework`, **but** in `risc0-circuit-rv32im 4.0.5` (the RISC-V circuit used by risc0 3.0.x) the Metal segment prover is disabled in source (`src/prove/mod.rs` lines 49-50: the `metal::segment_prover` branch is commented out; non-CUDA targets fall through to `hal::cpu::segment_prover()`). So **all STARK proving here ran on the CPU** (multi-threaded; `/usr/bin/time` shows ~8-9x user/real ratio on the 10-core M4). No GPU was used. Numbers would be substantially lower on a CUDA machine.
- `RISC0_DEV_MODE=0` was set explicitly for every run; the receipts are real (`InnerReceipt::Composite` / `InnerReceipt::Succinct`, never `Fake`), and every receipt was verified against the image ID with `receipt.verify(IMAGE_ID)` after proving.

## Results 1 — executor only (cycle counts), accelerated `k256` fork

`host exec accel 1 3 5 7 10 16 32` (log `logs/10_exec_accel.log`). `user_cycles` = `SessionInfo::cycles()` (sum of per-segment user cycles, no padding); `padded_cycles` = sum of 2^po2 over segments (what actually gets proven; max segment po2 = 20 by default).

| M | user_cycles | user cycles / sig | padded_cycles (sum 2^po2) | segments (po2) | exec wall s |
|---|---|---|---|---|---|
| 1 | 306,497 | 306,497 | 524,288 | 1 (19) | 0.06 |
| 3 | 897,786 | 299,262 | 1,048,576 | 1 (20) | 0.05 |
| 5 | 1,493,886 | 298,777 | 2,097,152 | 2 (20,20) | 0.06 |
| 7 | 2,086,332 | 298,047 | 2,359,296 | 3 (20,20,18) | 0.07 |
| 10 | 2,980,826 | 298,083 | 3,211,264 | 4 (20,20,20,16) | 0.09 |
| 16 | 4,754,287 | 297,143 | 5,242,880 | 5 (20 x5) | 0.13 |
| 32 | 9,512,524 | 297,266 | 10,485,760 | 10 (20 x10) | 0.23 |

Linear fit: ~**295.7k user cycles per BIP-340 verification** + ~10k fixed overhead (slope from M=1 to M=32: (9,512,524 - 306,497)/31 = 296,969). From the `RISC0_INFO` breakdown (prove logs), ~43% of user cycles for M>=10 are inside the `BigInt` (bigint2 modular-arithmetic) precompile: M=16 -> 29,616 BigInt ecalls, 2,209,118 cycles (1,851 ecalls per verification).

## Results 2 — executor only, stock crates.io `k256 0.13.4` (no precompiles)

`host exec stock 1 3 5 7 10 16 32` (log `logs/11_exec_stock.log`).

| M | user_cycles | user cycles / sig | padded_cycles | segments | exec wall s |
|---|---|---|---|---|---|
| 1 | 5,428,119 | 5,428,119 | 5,767,168 | 6 | 0.08 |
| 3 | 16,268,589 | 5,422,863 | 17,301,504 | 17 | 0.16 |
| 5 | 27,109,059 | 5,421,812 | 28,573,696 | 28 | 0.24 |
| 7 | 37,949,529 | 5,421,361 | 39,911,424 | 39 | 0.33 |
| 10 | 54,210,234 | 5,421,023 | 57,147,392 | 55 | 0.46 |
| 16 | 86,731,644 | 5,420,728 | 91,226,112 | 87 | 0.71 |
| 32 | 173,455,404 | 5,420,481 | 182,452,224 | 174 | 1.41 |

~**5.42M user cycles per verification** with stock k256, i.e. the risc0 fork (bigint2 precompile) gives a **~18.3x cycle reduction** (5,420,481 / 296,969) for BIP-340 verification.

## Results 3 — real proving (STARK), accelerated guest, CPU-only on M4

`RISC0_DEV_MODE=0 RISC0_INFO=1 RUST_LOG=info /usr/bin/time -l ./target/release/host prove accel <M>` (logs `logs/20_prove_accel_M*.log`). Receipt size = `bincode::serialize(&receipt).len()`. Peak RSS = `maximum resident set size` from `/usr/bin/time -l` (covers host + the child r0vm it waits on). Wall time is the `prove_with_opts` call only (executor + proving; excludes process start and key generation). Each M is a single run (no repetitions), fresh random keys each run.

| M | receipt kind | total (padded) cycles | user cycles | paging | reserved | segments | prove wall s | CPU user s | verify s | receipt bytes | peak RSS |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | Composite | 524,288 | 305,765 | 38,286 | 180,237 | 1 (2^19) | **48.4** | 404 | 0.011 | 268,194 (262 KB) | 3.58 GB |
| 5 | Composite | 2,097,152 | 1,493,826 | 69,072 | 534,254 | 2 (2^20 x2) | **206.1** | 1,806 | 0.024 | 562,756 (550 KB) | 7.69 GB |
| 10 | Composite | 3,178,496 | 2,974,267 | 106,508 | 97,721 | 4 (2^20 x3 + 2^16) | **338.6** | 3,021 | 0.044 | 1,053,512 (1.0 MB) | 7.94 GB |
| 16 | Composite | 5,242,880 | 4,763,726 | 151,026 | 328,128 | 5 (2^20 x5) | **586.7** | 5,285 | 0.060 | 1,406,698 (1.34 MB) | 8.68 GB |
| 5 | **Succinct** (`ProverOpts::succinct()`) | 2,097,152 | 1,501,768 | 69,072 | 526,312 | 2 (2^20 x2) | **262.7** | 2,400 | 0.013 | 223,234 (218 KB) | 8.54 GB |

Observations:
- Proving throughput is ~**100-105 s per 2^20 segment** on this CPU (M=5: 2 segments 206 s; M=16: 5 segments 587 s), i.e. roughly 10k padded cycles/s. The 2^19 segment (M=1) took 48 s.
- Per signature (amortised, composite): M=1 48 s; M=5 41 s; M=10 34 s; M=16 37 s. The step function comes from segment padding to powers of two (po2 = 20 max): ~3.4 signatures fit in one 2^20 segment.
- Composite receipt size grows ~linearly with segment count (~270 KB per segment). The succinct receipt is a constant ~218 KB regardless of M and cost +57 s of recursion (2 lifts + 1 join, on CPU) over the composite proof for M=5.
- `Sha2` ecalls stay at 4 for every M, so the BIP-340 tagged challenge hash does not appear to go through the SHA-256 precompile in this configuration (it is a few thousand software cycles per signature; immaterial next to the EC math). Not investigated further.
- The `reserved_cycles` column (zkVM-internal overhead) is noisy between runs because segment boundaries move with the random inputs.

## Results 4 — real proving, stock k256 guest (comparison)

## Results 4 — real proving, stock crates.io `k256 0.13.4` (no precompiles)

Run on 25 October 2026 on the same machine and toolchain, `RISC0_DEV_MODE=0`:

```text
RESULT variant=stock M=1 receipt_kind=Composite(6 segments) total_cycles=5767168 user_cycles=5428119 segments=6 prove_wall_s=576.54 verify_wall_s=0.072 receipt_bytes=1674764
      576.62 real      4932.53 user        45.99 sys
          7907180544  maximum resident set size
```

One stock verification proves in 576.5 s against 48.4 s with the accelerated crate (about 12×),
with a 1.67 MB receipt against 268 kB. Log: `logs/22_prove_stock_M1.log`.
