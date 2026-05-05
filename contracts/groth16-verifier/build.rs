// Copyright 2026 RISC Zero, Inc.
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

use risc0_circuit_recursion::control_id::{ALLOWED_CONTROL_ROOT, BN254_IDENTITY_CONTROL_ID};
use std::{fs, path::Path};

fn main() {
    // Write the control IDs to a file for inclusion in the program while avoiding dependency on the risc0_circuit_recursion crate
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
}
