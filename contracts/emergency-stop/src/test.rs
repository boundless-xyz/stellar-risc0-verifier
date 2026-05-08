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

use super::*;
use soroban_risc0_mock_verifier::{RiscZeroMockVerifier, RiscZeroMockVerifierClient};
use soroban_risc0_verifier_interface::Receipt;
use soroban_sdk::{Address, Bytes, BytesN, Env, testutils::Address as _};

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

const MOCK_SELECTOR: [u8; 4] = [0x01, 0x02, 0x03, 0x04];

struct Setup {
    env: Env,
    guardian: Address,
    inner: Address,
    selector: BytesN<4>,
    client: RiscZeroVerifierEmergencyStopClient<'static>,
}

fn setup() -> Setup {
    let env = Env::default();
    env.mock_all_auths();

    let guardian = Address::generate(&env);
    let selector = BytesN::from_array(&env, &MOCK_SELECTOR);
    let inner = env.register(RiscZeroMockVerifier, (selector.clone(),));
    let wrapper = env.register(
        RiscZeroVerifierEmergencyStop,
        (inner.clone(), guardian.clone()),
    );
    let client = RiscZeroVerifierEmergencyStopClient::new(&env, &wrapper);

    Setup {
        env,
        guardian,
        inner,
        selector,
        client,
    }
}

/// Builds `selector || [0u8; 32]`. Paired with [`zero_digest`] this is a
/// valid receipt under [`RiscZeroMockVerifier::verify_integrity`] because
/// `keccak([0u8; 32]) == keccak([0u8; 32])` — no contract call needed for
/// the common zero-claim case.
fn seal_with_selector(env: &Env, selector: &BytesN<4>) -> Bytes {
    let mut seal = selector.to_array().to_vec();
    seal.extend_from_slice(&[0u8; 32]);
    Bytes::from_slice(env, &seal)
}

fn zero_digest(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

// -----------------------------------------------------------------------------
// Constructor / initial state
// -----------------------------------------------------------------------------

#[test]
fn constructor_sets_owner_verifier_and_unpaused() {
    let s = setup();
    assert_eq!(s.client.get_owner(), Some(s.guardian));
    assert_eq!(s.client.verifier(), s.inner);
    assert!(!s.client.paused());
}

#[test]
fn selector_forwards_to_inner() {
    let s = setup();
    assert_eq!(s.client.selector(), s.selector);
}

// -----------------------------------------------------------------------------
// Forwarding while not paused
// -----------------------------------------------------------------------------

#[test]
fn verify_forwards_to_inner() {
    let s = setup();
    let mock = RiscZeroMockVerifierClient::new(&s.env, &s.inner);
    let image_id = BytesN::from_array(&s.env, &[0u8; 32]);
    let journal = BytesN::from_array(&s.env, &[1u8; 32]);
    let receipt = mock.mock_prove(&image_id, &journal);

    // RiscZeroVerifierClient is panic-on-error; a successful return here
    // proves the inner verifier was invoked and returned Ok.
    s.client.verify(&receipt.seal, &image_id, &journal);
}

#[test]
fn verify_integrity_forwards_to_inner() {
    let s = setup();
    let receipt = Receipt {
        seal: seal_with_selector(&s.env, &s.selector),
        claim_digest: zero_digest(&s.env),
    };

    s.client.verify_integrity(&receipt);
}

// -----------------------------------------------------------------------------
// Owner-gated estop
// -----------------------------------------------------------------------------

#[test]
fn estop_pauses_contract() {
    let s = setup();
    s.client.estop();
    assert!(s.client.paused());
}

#[test]
#[should_panic]
fn estop_non_owner_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let guardian = Address::generate(&env);
    let selector = BytesN::from_array(&env, &MOCK_SELECTOR);
    let inner = env.register(RiscZeroMockVerifier, (selector,));
    let wrapper = env.register(RiscZeroVerifierEmergencyStop, (inner, guardian));
    let client = RiscZeroVerifierEmergencyStopClient::new(&env, &wrapper);

    env.set_auths(&[]);
    client.estop();
}

#[test]
#[should_panic]
fn estop_already_paused_panics() {
    let s = setup();
    s.client.estop();
    s.client.estop();
}

// -----------------------------------------------------------------------------
// Pause-gating effects
// -----------------------------------------------------------------------------

#[test]
#[should_panic]
fn verify_after_estop_panics() {
    let s = setup();
    s.client.estop();

    let seal = seal_with_selector(&s.env, &s.selector);
    let image_id = BytesN::from_array(&s.env, &[0u8; 32]);
    let journal = BytesN::from_array(&s.env, &[1u8; 32]);
    s.client.verify(&seal, &image_id, &journal);
}

#[test]
#[should_panic]
fn verify_integrity_after_estop_panics() {
    let s = setup();
    s.client.estop();

    let receipt = Receipt {
        seal: seal_with_selector(&s.env, &s.selector),
        claim_digest: zero_digest(&s.env),
    };
    s.client.verify_integrity(&receipt);
}

#[test]
fn selector_still_works_after_estop() {
    let s = setup();
    s.client.estop();
    // Selector lookup must remain accessible so router admins can clean up
    // dead entries.
    assert_eq!(s.client.selector(), s.selector);
}

// -----------------------------------------------------------------------------
// Permissionless estop_with_proof
// -----------------------------------------------------------------------------

#[test]
fn estop_with_proof_rejects_non_zero_digest() {
    let s = setup();
    let receipt = Receipt {
        seal: seal_with_selector(&s.env, &s.selector),
        claim_digest: BytesN::from_array(&s.env, &[1u8; 32]),
    };
    let result = s.client.try_estop_with_proof(&receipt);
    match result {
        Err(Ok(e)) => assert_eq!(e, EmergencyStopError::InvalidProofOfExploit),
        other => panic!("expected InvalidProofOfExploit, got {:?}", other),
    }
    assert!(!s.client.paused());
}

#[test]
fn estop_with_proof_pauses_on_valid_zero_digest() {
    let s = setup();
    let receipt = Receipt {
        seal: seal_with_selector(&s.env, &s.selector),
        claim_digest: zero_digest(&s.env),
    };
    s.client.estop_with_proof(&receipt);
    assert!(s.client.paused());
}

#[test]
#[should_panic]
fn estop_with_proof_panics_when_inner_rejects() {
    let s = setup();

    // claim_digest = zero passes the upfront check, but the seal payload
    // (32 non-zero bytes) hashes to something other than keccak([0u8; 32]),
    // so the inner mock returns InvalidProof.
    let mut tampered = s.selector.to_array().to_vec();
    tampered.extend_from_slice(&[0xAAu8; 32]);
    let receipt = Receipt {
        seal: Bytes::from_slice(&s.env, &tampered),
        claim_digest: zero_digest(&s.env),
    };
    s.client.estop_with_proof(&receipt);
}

#[test]
fn estop_with_proof_inner_error_does_not_misdecode() {
    // Test for error-code disjointness.
    let s = setup();

    let wrong_selector = BytesN::from_array(&s.env, &[0xFF, 0xEE, 0xDD, 0xCC]);
    let receipt = Receipt {
        seal: seal_with_selector(&s.env, &wrong_selector),
        claim_digest: zero_digest(&s.env),
    };

    let result = s.client.try_estop_with_proof(&receipt);
    assert!(
        matches!(result, Err(Err(_))),
        "inner VerifierError must not decode as EmergencyStopError, got {:?}",
        result
    );
    assert!(!s.client.paused());
}

#[test]
#[should_panic]
fn estop_with_proof_panics_when_already_paused() {
    let s = setup();
    s.client.estop();

    let receipt = Receipt {
        seal: seal_with_selector(&s.env, &s.selector),
        claim_digest: zero_digest(&s.env),
    };
    s.client.estop_with_proof(&receipt);
}

// -----------------------------------------------------------------------------
// Pausable trait surface (one-way invariant)
// -----------------------------------------------------------------------------

#[test]
#[should_panic]
fn pause_via_trait_panics() {
    let s = setup();
    s.client.pause(&s.guardian);
}

#[test]
#[should_panic]
fn unpause_via_trait_panics_when_unpaused() {
    let s = setup();
    s.client.unpause(&s.guardian);
}

#[test]
#[should_panic]
fn unpause_via_trait_panics_when_paused() {
    let s = setup();
    s.client.estop();
    s.client.unpause(&s.guardian);
}

// -----------------------------------------------------------------------------
// Ownership transfer (sanity check that stellar-access integration works)
// -----------------------------------------------------------------------------

#[test]
fn ownership_can_transfer_two_step() {
    let s = setup();
    let new_owner = Address::generate(&s.env);

    // Step 1: current owner initiates transfer with a TTL.
    let live_until = s.env.ledger().sequence().saturating_add(1000);
    s.client.transfer_ownership(&new_owner, &live_until);

    // Step 2: new owner accepts.
    s.client.accept_ownership();

    assert_eq!(s.client.get_owner(), Some(new_owner.clone()));

    // New owner can call estop; old owner cannot (auth-gated).
    s.client.estop();
    assert!(s.client.paused());
}
