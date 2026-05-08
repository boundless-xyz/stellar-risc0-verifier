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
    Receipt, RiscZeroSelectorClient, RiscZeroSelectorInterface, RiscZeroVerifierClient,
    RiscZeroVerifierInterface, VerifierError,
};
use soroban_sdk::{
    Address, Bytes, BytesN, Env, contract, contracterror, contractimpl, contracttype,
    panic_with_error,
};
use stellar_access::ownable::{Ownable, set_owner};
use stellar_contract_utils::pausable::{self, Pausable};
use stellar_macros::{only_owner, when_not_paused};

/// Errors specific to the emergency-stop wrapper.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EmergencyStopError {
    /// Returned when `estop_with_proof` is called with a non-zero claim
    /// digest. The permissionless circuit-breaker requires a receipt that
    /// commits to the all-zeroes claim digest, since such a receipt can
    /// only exist if the proof system is critically broken.
    InvalidProofOfExploit = 100,

    /// `Pausable::pause` is rejected on this wrapper. Use `estop` (owner) or
    /// `estop_with_proof` (permissionless circuit-breaker) instead.
    UseEstopInstead = 101,

    /// `Pausable::unpause` is rejected on this wrapper. The contract is
    /// one-way: once stopped, it cannot be restarted.
    CannotUnpause = 102,
}

#[contracttype]
enum DataKey {
    InnerVerifier,
}

const ZERO_DIGEST: [u8; 32] = [0u8; 32];

/// Emergency-stop wrapper for a [`RiscZeroVerifierInterface`] contract.
///
/// Mirrors the upstream Solidity `RiscZeroVerifierEmergencyStop`. The
/// wrapper forwards `verify` and `verify_integrity` calls to the inner
/// verifier while the contract is unpaused, and rejects them once
/// stopped. Once stopped, this contract can never be restarted.
#[contract]
pub struct RiscZeroVerifierEmergencyStop;

#[contractimpl]
impl RiscZeroVerifierEmergencyStop {
    /// Initializes the wrapper.
    ///
    /// `verifier` is the inner verifier contract whose calls this wrapper
    /// gates. `guardian` becomes the owner and may call [`Self::estop`].
    pub fn __constructor(env: Env, verifier: Address, guardian: Address) {
        set_owner(&env, &guardian);
        env.storage()
            .instance()
            .set(&DataKey::InnerVerifier, &verifier);
    }

    /// Returns the inner verifier address.
    pub fn verifier(env: Env) -> Address {
        Self::read_inner(&env)
    }

    /// Owner-gated emergency stop. Pauses the wrapper permanently.
    #[only_owner]
    pub fn estop(env: Env) {
        pausable::pause(&env);
    }

    /// Permissionless circuit-breaker.
    pub fn estop_with_proof(env: Env, receipt: Receipt) -> Result<(), EmergencyStopError> {
        if receipt.claim_digest != BytesN::from_array(&env, &ZERO_DIGEST) {
            return Err(EmergencyStopError::InvalidProofOfExploit);
        }
        let _ = Self::verify_integrity(env.clone(), receipt);
        pausable::pause(&env);
        Ok(())
    }

    fn read_inner(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::InnerVerifier)
            .expect("inner verifier set in constructor")
    }
}

#[contractimpl]
impl RiscZeroVerifierInterface for RiscZeroVerifierEmergencyStop {
    #[when_not_paused]
    fn verify(
        env: Env,
        seal: Bytes,
        image_id: BytesN<32>,
        journal: BytesN<32>,
    ) -> Result<(), VerifierError> {
        let inner = Self::read_inner(&env);
        RiscZeroVerifierClient::new(&env, &inner).verify(&seal, &image_id, &journal);
        Ok(())
    }

    #[when_not_paused]
    fn verify_integrity(env: Env, receipt: Receipt) -> Result<(), VerifierError> {
        let inner = Self::read_inner(&env);
        RiscZeroVerifierClient::new(&env, &inner).verify_integrity(&receipt);
        Ok(())
    }
}

#[contractimpl]
impl RiscZeroSelectorInterface for RiscZeroVerifierEmergencyStop {
    fn selector(env: Env) -> BytesN<4> {
        let inner = Self::read_inner(&env);
        RiscZeroSelectorClient::new(&env, &inner).selector()
    }
}

#[contractimpl(contracttrait)]
impl Pausable for RiscZeroVerifierEmergencyStop {
    fn paused(e: &Env) -> bool {
        pausable::paused(e)
    }

    fn pause(e: &Env, _caller: Address) {
        panic_with_error!(e, EmergencyStopError::UseEstopInstead);
    }

    fn unpause(e: &Env, _caller: Address) {
        panic_with_error!(e, EmergencyStopError::CannotUnpause);
    }
}

#[contractimpl(contracttrait)]
impl Ownable for RiscZeroVerifierEmergencyStop {}

#[cfg(test)]
mod test;
