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

use soroban_sdk::{BytesN, Env};

/// Splits a digest into two 32-byte parts after reversing byte order.
///
/// This function reverses the byte order of the input digest and splits it into
/// two 32-byte values (zero-padded on the left), matching Solidity's convention
/// where claim_0 gets the upper 128 bits and claim_1 gets the lower 128 bits.
///
/// # Parameters
///
/// - `digest`: A 32-byte digest to split
///
/// # Returns
///
/// A tuple of two 32-byte values: (upper 128 bits, lower 128 bits) zero-padded
pub(crate) fn split_digest(env: &Env, digest: BytesN<32>) -> (BytesN<32>, BytesN<32>) {
    // Get the digest as a byte array
    let mut bytes = digest.to_array();

    // Reverse the byte order (equivalent to reverseByteOrderUint256)
    bytes.reverse();

    // Split into two 16-byte parts and convert to 32-byte (zero-padded on left)
    // Note: Solidity assigns upper bits to claim_0, lower bits to claim_1
    let mut claim_0 = [0u8; 32];
    let mut claim_1 = [0u8; 32];

    // Copy the upper 16 bytes to claim_0 (zero-pad left)
    claim_0[16..32].copy_from_slice(&bytes[16..32]);
    // Copy the lower 16 bytes to claim_1 (zero-pad left)
    claim_1[16..32].copy_from_slice(&bytes[0..16]);

    (
        BytesN::from_array(env, &claim_0),
        BytesN::from_array(env, &claim_1),
    )
}
