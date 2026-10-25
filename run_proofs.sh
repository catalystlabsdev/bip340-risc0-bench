#!/bin/bash
export PATH=$HOME/.risc0/bin:$HOME/.cargo/bin:$PATH
export RISC0_DEV_MODE=0 RISC0_INFO=1 RUST_LOG=info
cd "$(dirname "$0")"
for M in 5 10 16; do
  echo "=== START prove accel M=$M $(date)"
  /usr/bin/time -l ./target/release/host prove accel $M > logs/20_prove_accel_M${M}.log 2>&1
  echo "=== END M=$M rc=$? $(date)"
  grep -E 'RESULT|real|maximum resident' logs/20_prove_accel_M${M}.log
done
echo "=== START prove-succinct accel M=5 $(date)"
/usr/bin/time -l ./target/release/host prove-succinct accel 5 > logs/21_prove_succinct_accel_M5.log 2>&1
echo "=== END succinct rc=$? $(date)"
grep -E 'RESULT|real|maximum resident' logs/21_prove_succinct_accel_M5.log
echo ALLDONE
