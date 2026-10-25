use k256::schnorr::{signature::hazmat::PrehashSigner, SigningKey};
use methods::{VERIFY_SIGS_ELF, VERIFY_SIGS_ID, VERIFY_SIGS_STOCK_ELF, VERIFY_SIGS_STOCK_ID};
use rand_core::OsRng;
use risc0_zkvm::{default_executor, default_prover, ExecutorEnv, ProverOpts};
use std::time::Instant;

fn make_env(m: usize) -> ExecutorEnv<'static> {
    let msg = [0x42u8; 32];
    let mut pubkeys: Vec<[u8; 32]> = Vec::with_capacity(m);
    let mut sigs: Vec<u8> = Vec::with_capacity(64 * m);
    for _ in 0..m {
        let sk = SigningKey::random(&mut OsRng);
        let vk = sk.verifying_key();
        let sig = sk.sign_prehash(&msg).unwrap();
        pubkeys.push(vk.to_bytes().into());
        sigs.extend_from_slice(&sig.to_bytes());
    }
    ExecutorEnv::builder()
        .write(&msg).unwrap()
        .write(&pubkeys).unwrap()
        .write(&sigs).unwrap()
        .build()
        .unwrap()
}

fn elf_for(variant: &str) -> (&'static [u8], [u32; 8]) {
    match variant {
        "accel" => (VERIFY_SIGS_ELF, VERIFY_SIGS_ID),
        "stock" => (VERIFY_SIGS_STOCK_ELF, VERIFY_SIGS_STOCK_ID),
        _ => panic!("variant must be accel|stock"),
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("exec");
    let variant = args.get(2).map(|s| s.as_str()).unwrap_or("accel");
    let (elf, image_id) = elf_for(variant);
    let ms: Vec<usize> = args[3..].iter().map(|s| s.parse().unwrap()).collect();

    match mode {
        "exec" => {
            println!("variant={variant}");
            println!("| M | user_cycles | padded_cycles (sum 2^po2) | segments | segment po2s | exec_wall_s |");
            println!("|---|---|---|---|---|---|");
            for &m in &ms {
                let env = make_env(m);
                let t = Instant::now();
                let session = default_executor().execute(env, elf).unwrap();
                let wall = t.elapsed().as_secs_f64();
                let out: u32 = session.journal.decode().unwrap();
                assert_eq!(out as usize, m);
                let padded: u64 = session.segments.iter().map(|s| 1u64 << s.po2).sum();
                let po2s: Vec<String> = session.segments.iter().map(|s| s.po2.to_string()).collect();
                println!(
                    "| {} | {} | {} | {} | {} | {:.2} |",
                    m, session.cycles(), padded, session.segments.len(), po2s.join(","), wall
                );
            }
        }
        "prove" | "prove-succinct" => {
            let m = ms[0];
            let opts = if mode == "prove-succinct" { ProverOpts::succinct() } else { ProverOpts::default() };
            println!("variant={variant} M={m} opts={:?} RISC0_DEV_MODE={:?}", opts.receipt_kind, std::env::var("RISC0_DEV_MODE"));
            let env = make_env(m);
            let t = Instant::now();
            let info = default_prover().prove_with_opts(env, elf, &opts).unwrap();
            let wall = t.elapsed().as_secs_f64();
            let receipt = info.receipt;
            let bytes = bincode::serialize(&receipt).unwrap();
            let out: u32 = receipt.journal.decode().unwrap();
            assert_eq!(out as usize, m);
            let tv = Instant::now();
            receipt.verify(image_id).unwrap();
            let vwall = tv.elapsed().as_secs_f64();
            let kind = match &receipt.inner {
                risc0_zkvm::InnerReceipt::Composite(c) => format!("Composite({} segments)", c.segments.len()),
                risc0_zkvm::InnerReceipt::Succinct(_) => "Succinct".to_string(),
                risc0_zkvm::InnerReceipt::Groth16(_) => "Groth16".to_string(),
                risc0_zkvm::InnerReceipt::Fake(_) => "FAKE(dev mode!)".to_string(),
                _ => "other".to_string(),
            };
            println!(
                "RESULT variant={} M={} receipt_kind={} total_cycles={} user_cycles={} segments={} prove_wall_s={:.2} verify_wall_s={:.3} receipt_bytes={}",
                variant, m, kind, info.stats.total_cycles, info.stats.user_cycles, info.stats.segments, wall, vwall, bytes.len()
            );
        }
        _ => panic!("mode must be exec|prove|prove-succinct"),
    }
}
