use k256::schnorr::{signature::hazmat::PrehashVerifier, Signature, VerifyingKey};
use risc0_zkvm::guest::env;

fn main() {
    let msg: [u8; 32] = env::read();
    let pubkeys: Vec<[u8; 32]> = env::read();
    let sigs: Vec<u8> = env::read(); // 64 * M bytes
    let m = pubkeys.len();
    assert_eq!(sigs.len(), 64 * m, "signature bytes length mismatch");

    for i in 0..m {
        let vk = VerifyingKey::from_bytes(&pubkeys[i]).expect("invalid x-only pubkey");
        let sig = Signature::try_from(&sigs[i * 64..(i + 1) * 64]).expect("invalid signature encoding");
        // BIP-340 verification of a raw 32-byte message (prehash = the message itself).
        vk.verify_prehash(&msg, &sig).expect("BIP-340 signature verification failed");
    }

    env::commit(&(m as u32));
}
