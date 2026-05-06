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

#![doc = include_str!("../README.md")]

use risc0_zkvm::{InnerReceipt, sha::Digestible};

/// Errors produced by `encode_seal` when the receipt type is unsupported for
/// encoding.
pub enum Error {
    /// The receipt type is not supported for encoding into a seal for smart
    /// contract verifiers.
    UnsupportedReceiptType,
}

/// Encode the seal of the given receipt for use with smart contract verifiers.
///
/// Appends the verifier selector, determined from the first 4 bytes of the
/// verifier parameters including the Groth16 verification key and the control
/// IDs that commit to the RISC Zero circuits.
///
/// Panics:
/// - If the resulting encoded seal exceeds the platform maximum usize
pub fn encode_seal(receipt: &risc0_zkvm::Receipt) -> Result<Vec<u8>, Error> {
    let seal = match receipt.inner.clone() {
        InnerReceipt::Fake(receipt) => {
            let seal = receipt.claim.digest().as_bytes().to_vec();
            let selector = &[0xFFu8; 4];
            // Create a new vector with the capacity to hold both selector and seal
            let mut selector_seal = Vec::with_capacity(
                selector
                    .len()
                    .checked_add(seal.len())
                    .expect("Seal length overflow"),
            );
            selector_seal.extend_from_slice(selector);
            selector_seal.extend_from_slice(&seal);
            selector_seal
        }
        InnerReceipt::Groth16(receipt) => {
            let selector = &receipt.verifier_parameters.as_bytes()[..4];
            // Create a new vector with the capacity to hold both selector and seal
            let mut selector_seal = Vec::with_capacity(
                selector
                    .len()
                    .checked_add(receipt.seal.len())
                    .expect("Seal length overflow"),
            );
            selector_seal.extend_from_slice(selector);
            selector_seal.extend_from_slice(receipt.seal.as_ref());
            selector_seal
        }
        _ => return Err(Error::UnsupportedReceiptType),
    };
    Ok(seal)
}
