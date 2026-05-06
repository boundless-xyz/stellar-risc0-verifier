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
    Receipt, RiscZeroSelectorClient, RiscZeroVerifierClient, RiscZeroVerifierInterface,
    VerifierError,
};
use soroban_sdk::{
    Address, Bytes, BytesN, Env, contract, contractevent, contractimpl, contracttype,
};
use stellar_access::ownable::{Ownable, set_owner};
use stellar_macros::only_owner;

const DAY_IN_LEDGERS: u32 = 17_280;
const VERIFIER_EXTEND_AMOUNT: u32 = 90 * DAY_IN_LEDGERS;
const VERIFIER_TTL_THRESHOLD: u32 = VERIFIER_EXTEND_AMOUNT - DAY_IN_LEDGERS;

/// Router mapping entry for a verifier selector.
///
/// This enum represents the raw state stored in the router mapping:
/// - `Active(Address)` means the selector routes to that verifier contract.
/// - `Tombstone` means the selector was removed and can never be reused.
///
/// The router `verifiers` getter returns `None` when a selector has never been
/// set, allowing callers to distinguish "unset" vs "removed" without relying on
/// errors.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifierEntry {
    /// Active verifier for the selector.
    Active(Address),
    /// Selector is permanently removed.
    Tombstone,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    /// Selector-specific verifier entry.
    Verifier(BytesN<4>),
}

#[contractevent]
pub struct VerifierAdded {
    #[topic]
    selector: BytesN<4>,
    addr: Address,
}

#[contractevent]
pub struct VerifierRemoved {
    #[topic]
    selector: BytesN<4>,
}

#[contract]
/// Routes verification requests to selector-specific verifier contracts.
pub struct RiscZeroVerifierRouter;

#[contractimpl]
impl RiscZeroVerifierRouter {
    /// Initializes the router with the admin that can manage verifiers.
    pub fn __constructor(env: Env, owner: Address) {
        set_owner(&env, &owner);
    }

    /// Adds a verifier for the selector.
    #[only_owner]
    pub fn add_verifier(env: Env, verifier: Address) -> Result<(), VerifierError> {
        let selector = RiscZeroSelectorClient::new(&env, &verifier).selector();

        let key = DataKey::Verifier(selector.clone());
        let verifier_address: Option<VerifierEntry> = Self::read_verifier_entry(&env, &key);

        if let Some(entry) = verifier_address {
            match entry {
                VerifierEntry::Tombstone => return Err(VerifierError::SelectorRemoved),
                VerifierEntry::Active(_) => return Err(VerifierError::SelectorInUse),
            }
        }

        env.storage()
            .persistent()
            .set(&key, &VerifierEntry::Active(verifier.clone()));

        VerifierAdded {
            selector,
            addr: verifier,
        }
        .publish(&env);

        Ok(())
    }

    /// Removes a verifier for the selector, marking it as permanently removed.
    #[only_owner]
    pub fn remove_verifier(env: Env, selector: BytesN<4>) -> Result<(), VerifierError> {
        let key = DataKey::Verifier(selector.clone());
        match Self::read_verifier_entry(&env, &key) {
            Some(VerifierEntry::Tombstone) => return Err(VerifierError::SelectorRemoved),
            Some(VerifierEntry::Active(_)) => (),
            None => return Err(VerifierError::SelectorUnknown),
        }
        env.storage()
            .persistent()
            .set(&key, &VerifierEntry::Tombstone);

        VerifierRemoved { selector }.publish(&env);

        Ok(())
    }

    /// Returns the raw verifier entry for a selector (unset, active, or
    /// tombstone).
    pub fn verifiers(env: Env, selector: BytesN<4>) -> Option<VerifierEntry> {
        let key = DataKey::Verifier(selector);
        Self::read_verifier_entry(&env, &key)
    }

    /// Returns the verifier for a selector.
    pub fn get_verifier_by_selector(
        env: Env,
        selector: BytesN<4>,
    ) -> Result<Address, VerifierError> {
        Self::get_verifier(&env, &selector)
    }

    /// Returns the verifier for a selector.
    fn get_verifier(env: &Env, selector: &BytesN<4>) -> Result<Address, VerifierError> {
        let key = DataKey::Verifier(selector.clone());
        let verifier_address: Option<VerifierEntry> = Self::read_verifier_entry(env, &key);

        match verifier_address {
            Some(VerifierEntry::Tombstone) => Err(VerifierError::SelectorRemoved),
            Some(VerifierEntry::Active(address)) => Ok(address),
            None => Err(VerifierError::SelectorUnknown),
        }
    }

    /// Reads the verifier entry and refreshes its TTL using the router policy
    /// when present.
    fn read_verifier_entry(env: &Env, key: &DataKey) -> Option<VerifierEntry> {
        env.storage().persistent().get(key).inspect(|_| {
            env.storage().persistent().extend_ttl(
                key,
                VERIFIER_TTL_THRESHOLD,
                VERIFIER_EXTEND_AMOUNT,
            );
        })
    }
}

#[contractimpl]
impl RiscZeroVerifierInterface for RiscZeroVerifierRouter {
    /// Verifies a receipt from its components.
    fn verify(
        env: Env,
        seal: Bytes,
        image_id: BytesN<32>,
        journal: BytesN<32>,
    ) -> Result<(), VerifierError> {
        let selector = selector_from_seal(&seal)?;
        let verifier = Self::get_verifier(&env, &selector)?;
        let verifier = RiscZeroVerifierClient::new(&env, &verifier);
        verifier.verify(&seal, &image_id, &journal);
        Ok(())
    }

    /// Verifies receipt integrity using the selector's verifier.
    fn verify_integrity(env: Env, receipt: Receipt) -> Result<(), VerifierError> {
        let selector = selector_from_seal(&receipt.seal)?;
        let verifier = Self::get_verifier(&env, &selector)?;
        let verifier = RiscZeroVerifierClient::new(&env, &verifier);
        verifier.verify_integrity(&receipt);
        Ok(())
    }
}

/// Extracts the 4-byte selector from the seal prefix.
fn selector_from_seal(seal: &Bytes) -> Result<BytesN<4>, VerifierError> {
    if seal.len() < 4 {
        return Err(VerifierError::MalformedSeal);
    }
    seal.slice(0..4)
        .try_into()
        .map_err(|_| VerifierError::MalformedSeal)
}

#[contractimpl(contracttrait)]
impl Ownable for RiscZeroVerifierRouter {}

#[cfg(test)]
mod test;
