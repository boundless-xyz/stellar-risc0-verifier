#![no_std]

use risc0_interface::{Receipt, RiscZeroVerifierClient, RiscZeroVerifierInterface, VerifierError};
use soroban_sdk::{Address, Bytes, BytesN, Env, contract, contractimpl, contracttype};
use stellar_access::ownable::{Ownable, set_owner};
use stellar_macros::only_owner;

#[cfg(test)]
mod test;

/// Router mapping entry for a verifier selector.
///
/// This enum represents the raw state stored in the router mapping:
/// - `Active(Address)` means the selector routes to that verifier contract.
/// - `Tombstone` means the selector was removed and can never be reused.
///
/// The router `verifiers` getter returns `None` when a selector has never been set,
/// allowing callers to distinguish "unset" vs "removed" without relying on errors.
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
    pub fn add_verifier(
        env: Env,
        selector: BytesN<4>,
        verifier: Address,
    ) -> Result<(), VerifierError> {
        let key = DataKey::Verifier(selector);
        let verifier_address: Option<VerifierEntry> = env.storage().instance().get(&key);

        if let Some(entry) = verifier_address {
            match entry {
                VerifierEntry::Tombstone => return Err(VerifierError::SelectorRemoved),
                VerifierEntry::Active(_) => return Err(VerifierError::SelectorInUse),
            }
        }

        env.storage()
            .instance()
            .set(&key, &VerifierEntry::Active(verifier));

        Ok(())
    }

    /// Removes a verifier for the selector, marking it as permanently removed.
    #[only_owner]
    pub fn remove_verifier(env: Env, selector: BytesN<4>) -> Result<(), VerifierError> {
        let key = DataKey::Verifier(selector);
        let verifier_address: Option<VerifierEntry> = env.storage().instance().get(&key);

        if verifier_address.is_none() {
            return Err(VerifierError::SelectorUnknown);
        }

        env.storage()
            .instance()
            .set(&key, &VerifierEntry::Tombstone);

        Ok(())
    }

    /// Returns the raw verifier entry for a selector (unset, active, or
    /// tombstone).
    pub fn verifiers(env: Env, selector: BytesN<4>) -> Option<VerifierEntry> {
        let key = DataKey::Verifier(selector);
        env.storage().instance().get(&key)
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
        let verifier_address: Option<VerifierEntry> = env.storage().instance().get(&key);

        match verifier_address {
            Some(VerifierEntry::Tombstone) => Err(VerifierError::SelectorRemoved),
            Some(VerifierEntry::Active(address)) => Ok(address),
            None => Err(VerifierError::SelectorUnknown),
        }
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
    Ok(seal.slice(0..4).try_into().unwrap())
}

#[contractimpl(contracttrait)]
impl Ownable for RiscZeroVerifierRouter {}
