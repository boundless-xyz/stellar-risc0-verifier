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

use risc0_binfmt::tagged_struct;
use risc0_zkp::core::{
    digest::Digest,
    hash::sha::{Sha256, cpu::Impl as ShaImpl},
};
use std::{fs, path::Path};

fn main() {
    // Define constants in rust module
    fs::write(
        Path::new(&std::env::var("OUT_DIR").unwrap()).join("tag_digests.rs"),
        format!(
            r#"
const RECEIPT_CLAIM_TAG_DIGEST: [u8; 32] = {:?};
const OUTPUT_TAG_DIGEST: [u8; 32] = {:?};
const HALTED_SYSTEM_STATE_DIGEST: [u8; 32] = {:?};
"#,
            ShaImpl::hash_bytes(b"risc0.ReceiptClaim").as_bytes(),
            ShaImpl::hash_bytes(b"risc0.Output").as_bytes(),
            tagged_struct::<ShaImpl>("risc0.SystemState", &[Digest::ZERO], &[0u32]).as_bytes(),
        ),
    )
    .unwrap();
}
