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

use crate::types::VerificationKey;
use soroban_sdk::{
    Env,
    crypto::bn254::{Bn254G1Affine as G1Affine, Bn254G2Affine as G2Affine},
};

// Control IDs generated from build.rs from the risc0-circuit-recursion crate.
include!(concat!(env!("OUT_DIR"), "/control_ids.rs"));

// Selector computed from control IDs and verification key digest, generated
// in build.rs.
include!(concat!(env!("OUT_DIR"), "/selector.rs"));

// Verification key bytes (ALPHA, BETA, GAMMA, DELTA, IC) — extracted from
// `risc0_groth16::verifying_key()` in build.rs and re-encoded into Soroban's
// affine point format. The trusted-setup parameters are not duplicated in
// this repo; they live in upstream and flow through here at compile time.
include!(concat!(env!("OUT_DIR"), "/vk.rs"));

/// Verification key for the Groth16 receipts produced by RISC Zero's recursion
/// prover, sourced at build time from `risc0_groth16::verifying_key()`.
pub fn verification_key(env: &Env) -> VerificationKey {
    VerificationKey {
        alpha: G1Affine::from_array(env, &ALPHA),
        beta: G2Affine::from_array(env, &BETA),
        gamma: G2Affine::from_array(env, &GAMMA),
        delta: G2Affine::from_array(env, &DELTA),
        ic: [
            G1Affine::from_array(env, &IC[0]),
            G1Affine::from_array(env, &IC[1]),
            G1Affine::from_array(env, &IC[2]),
            G1Affine::from_array(env, &IC[3]),
            G1Affine::from_array(env, &IC[4]),
            G1Affine::from_array(env, &IC[5]),
        ],
    }
}
