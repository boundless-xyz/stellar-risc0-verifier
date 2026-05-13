// Copyright 2026 Boundless Foundation, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use ark_bn254::Bn254;
use ark_ff::{BigInteger, PrimeField};
use ark_groth16::VerifyingKey;
use ark_serialize::CanonicalDeserialize;
use risc0_binfmt::{Digestible, tagged_struct};
use risc0_circuit_recursion::control_id::{ALLOWED_CONTROL_ROOT, BN254_IDENTITY_CONTROL_ID};
use risc0_groth16::verifying_key;
use risc0_zkp::core::hash::sha::cpu::Impl as ShaImpl;
use std::{fs, path::Path};

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();

    // Write the proof system digests
    fs::write(
        Path::new(&out_dir).join("control_ids.rs"),
        format!(
            r#"
pub const ALLOWED_CONTROL_ROOT: [u8; 32] = {:?};
pub const BN254_IDENTITY_CONTROL_ID: [u8; 32] = {:?};
"#,
            ALLOWED_CONTROL_ROOT.as_bytes().to_vec(),
            BN254_IDENTITY_CONTROL_ID
                .as_bytes()
                .iter()
                .rev()
                .cloned()
                .collect::<Vec<u8>>()
        ),
    )
    .unwrap();

    // Extract the inner ark verification key from the upstream wrapper.
    let upstream_vk = verifying_key();
    let json = serde_json::to_vec(&upstream_vk).expect("serialize upstream vk");
    let ark_bytes: Vec<u8> =
        serde_json::from_slice(&json).expect("decode upstream vk into Vec<u8>");
    let inner = VerifyingKey::<Bn254>::deserialize_uncompressed(ark_bytes.as_slice())
        .expect("decode arkworks VerifyingKey");

    // Write the verification key values
    fs::write(
        Path::new(&out_dir).join("vk.rs"),
        format!(
            r#"
pub const ALPHA: [u8; 64] = {:?};
pub const BETA: [u8; 128] = {:?};
pub const GAMMA: [u8; 128] = {:?};
pub const DELTA: [u8; 128] = {:?};
pub const IC: [[u8; 64]; {}] = {:?};
"#,
            encode_g1(&inner.alpha_g1),
            encode_g2(&inner.beta_g2),
            encode_g2(&inner.gamma_g2),
            encode_g2(&inner.delta_g2),
            inner.gamma_abc_g1.len(),
            inner
                .gamma_abc_g1
                .iter()
                .map(encode_g1)
                .collect::<Vec<[u8; 64]>>(),
        ),
    )
    .unwrap();

    // Compute the verification key
    let vk_digest = upstream_vk.digest::<ShaImpl>();
    let selector_full = tagged_struct::<ShaImpl>(
        "risc0.Groth16ReceiptVerifierParameters",
        &[ALLOWED_CONTROL_ROOT, BN254_IDENTITY_CONTROL_ID, vk_digest],
        &[],
    );
    let s = selector_full.as_bytes();
    let selector = [s[0], s[1], s[2], s[3]];

    // Write the selector
    fs::write(
        Path::new(&out_dir).join("selector.rs"),
        format!("\npub const SELECTOR: [u8; 4] = {:?};\n", selector),
    )
    .unwrap();
}

fn encode_g1(p: &ark_bn254::G1Affine) -> [u8; 64] {
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&p.x.into_bigint().to_bytes_be());
    out[32..].copy_from_slice(&p.y.into_bigint().to_bytes_be());
    out
}

fn encode_g2(p: &ark_bn254::G2Affine) -> [u8; 128] {
    let mut out = [0u8; 128];
    out[0..32].copy_from_slice(&p.x.c1.into_bigint().to_bytes_be());
    out[32..64].copy_from_slice(&p.x.c0.into_bigint().to_bytes_be());
    out[64..96].copy_from_slice(&p.y.c1.into_bigint().to_bytes_be());
    out[96..128].copy_from_slice(&p.y.c0.into_bigint().to_bytes_be());
    out
}
