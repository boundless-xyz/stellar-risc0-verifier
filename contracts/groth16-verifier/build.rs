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

use hex_literal::hex;
use risc0_circuit_recursion::control_id::{ALLOWED_CONTROL_ROOT, BN254_IDENTITY_CONTROL_ID};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

// SHA256 digest of the verification key, hashed using tagged struct encoding. See https://github.com/risc0/risc0-ethereum/blob/main/contracts/version-management-design.md
// Must be updated manually if the verification key changes however there is no
// reason to expect this to ever change (it would required another trusted
// setup)
pub const VK_DIGEST: [u8; 32] =
    hex!("21c5fdd9b4d576b17581f50b755482ba7a2134a3b5186e8e454acfa1f69511ab");

fn main() {
    // Write the control IDs to a file for inclusion in the program while avoiding
    // dependency on the risc0_circuit_recursion crate
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("control_ids.rs");
    fs::write(
        dest,
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

    let dest = Path::new(&out_dir).join("selector.rs");
    fs::write(
        dest,
        format!(
            r#"
pub const SELECTOR: [u8; 4] = {:?};
"#,
            compute_selector(
                ALLOWED_CONTROL_ROOT.into(),
                BN254_IDENTITY_CONTROL_ID.into(),
                VK_DIGEST
            )
        ),
    )
    .unwrap();
}

fn compute_selector(
    control_root: [u8; 32],
    bn254_control_id: [u8; 32],
    vk_digest: [u8; 32],
) -> [u8; 4] {
    let tag_struct = tagged_struct(
        "risc0.Groth16ReceiptVerifierParameters",
        &[control_root, bn254_control_id, vk_digest],
    );
    [tag_struct[0], tag_struct[1], tag_struct[2], tag_struct[3]]
}

// Tagged struct hashing strategy for computing the selector
fn tagged_struct(tag: &str, down: &[[u8; 32]]) -> [u8; 32] {
    let tag_digest = Sha256::digest(tag.as_bytes());

    let capacity = 32 + (down.len() * 32) + std::mem::size_of::<u16>();
    let mut tag_struct = Vec::with_capacity(capacity);
    tag_struct.extend_from_slice(&tag_digest);

    for digest in down {
        tag_struct.extend_from_slice(digest);
    }

    let down_count: u16 = down
        .len()
        .try_into()
        .expect("struct defined with more than 2^16 fields");
    tag_struct.extend_from_slice(&down_count.to_le_bytes());

    Sha256::digest(tag_struct).into()
}
