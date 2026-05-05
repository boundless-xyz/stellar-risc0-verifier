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

#![no_std]

use soroban_risc0_verifier_interface::{
    Receipt, ReceiptClaim, RiscZeroVerifierInterface, VerifierError,
};
use soroban_sdk::{Bytes, BytesN, Env, Vec, contract, contractimpl, crypto::bn254::Fr, vec};

use constants::{ALLOWED_CONTROL_ROOT, BN254_IDENTITY_CONTROL_ID, verification_key};
use types::{Groth16Proof, Groth16Seal};
use utils::split_digest;

mod constants;
mod types;
mod utils;

/// Groth16 verifier contract for RISC Zero receipts of execution.
///
/// This contract implements the [`RiscZeroVerifierInterface`] using Groth16
/// zero-knowledge proofs over the BN254 elliptic curve.
#[contract]
pub struct RiscZeroGroth16Verifier;

#[contractimpl]
impl RiscZeroGroth16Verifier {
    /// Returns the verifier's selector
    pub fn selector(env: Env) -> BytesN<4> {
        BytesN::from_array(&env, &constants::SELECTOR)
    }

    /// Verifies a Groth16 proof with the given public signals.
    ///
    /// This function implements the core Groth16 verification algorithm using
    /// the BN254 pairing-friendly elliptic curve. The verification checks
    /// the pairing equation:
    ///
    /// `e(-A, B) * e(alpha, beta) * e(vk_x, gamma) * e(C, delta) == 1`
    ///
    /// where `vk_x` is computed as a linear combination of the verification
    /// key's IC points weighted by the public signals.
    ///
    /// # Parameters
    ///
    /// - `proof`: The Groth16 proof containing points A, B, and C
    /// - `pub_signals`: Vector of public input signals (scalar field elements)
    pub fn verify_proof(
        env: Env,
        proof: Groth16Proof,
        pub_signals: Vec<Fr>,
    ) -> Result<bool, VerifierError> {
        let vk = verification_key(&env);
        let bn = env.crypto().bn254();

        if pub_signals.len() + 1 != vk.ic.len() as u32 {
            return Err(VerifierError::MalformedPublicInputs);
        }

        let mut vk_x = vk.ic[0].clone();
        for (s, v) in pub_signals.iter().zip(vk.ic.iter().skip(1)) {
            let prod = bn.g1_mul(v, &s);
            vk_x = bn.g1_add(&vk_x, &prod);
        }

        // Compute the pairing check:
        // e(-A, B) * e(alpha, beta) * e(vk_x, gamma) * e(C, delta) == 1
        let neg_a = -proof.a;
        let g1_points = vec![&env, neg_a, vk.alpha, vk_x, proof.c];
        let g2_points = vec![&env, proof.b, vk.beta, vk.gamma, vk.delta];

        Ok(bn.pairing_check(g1_points, g2_points))
    }
}

#[contractimpl]
impl RiscZeroVerifierInterface for RiscZeroGroth16Verifier {
    fn verify(
        env: Env,
        seal: Bytes,
        image_id: BytesN<32>,
        journal: BytesN<32>,
    ) -> Result<(), VerifierError> {
        let claim = ReceiptClaim::new(&env, image_id, journal);
        let receipt = Receipt {
            seal,
            claim_digest: claim.digest(&env),
        };
        Self::verify_integrity(env, receipt)
    }

    fn verify_integrity(env: Env, receipt: Receipt) -> Result<(), VerifierError> {
        let seal = Groth16Seal::try_from(receipt.seal)?;

        if seal.selector != constants::SELECTOR {
            return Err(VerifierError::InvalidSelector);
        }

        let (claim_0, claim_1) = split_digest(&env, receipt.claim_digest);
        let (control_root_0, control_root_1) =
            split_digest(&env, BytesN::from_array(&env, &ALLOWED_CONTROL_ROOT));

        // Convert BN254_CONTROL_ID to BytesN<32>
        let bn254_control_id: BytesN<32> = BytesN::from_array(&env, &BN254_IDENTITY_CONTROL_ID);

        // Create public signals as Fr field elements
        let mut pub_signals = Vec::new(&env);
        pub_signals.push_back(Fr::from_bytes(control_root_0));
        pub_signals.push_back(Fr::from_bytes(control_root_1));
        pub_signals.push_back(Fr::from_bytes(claim_0));
        pub_signals.push_back(Fr::from_bytes(claim_1));
        pub_signals.push_back(Fr::from_bytes(bn254_control_id));

        // Verify the proof and panic if invalid
        match Self::verify_proof(env, seal.proof, pub_signals)? {
            true => Ok(()),
            false => Err(VerifierError::InvalidProof),
        }
    }
}

#[cfg(test)]
mod test;
