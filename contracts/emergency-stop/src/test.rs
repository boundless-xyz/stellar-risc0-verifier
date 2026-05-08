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

/// Builds `selector || [0u8; 32]`. Paired with `zero_digest` this is a
/// valid receipt under [`RiscZeroMockVerifier::verify_integrity`] because
/// `keccak([0u8; 32]) == keccak([0u8; 32])` — no contract call needed.
fn seal_with_selector(env: &Env, selector: &BytesN<4>) -> Bytes {
    let mut seal = selector.to_array().to_vec();
    seal.extend_from_slice(&[0u8; 32]);
    Bytes::from_slice(env, &seal)
}

fn zero_digest(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[test]
fn constructor_initializes_state_and_accessors() {
    // Covers: __constructor, verifier(), selector() (forwards to inner),
    //         paused() initial state, get_owner() initial state.
    let s = setup();
    assert_eq!(s.client.get_owner(), Some(s.guardian));
    assert_eq!(s.client.verifier(), s.inner);
    assert_eq!(s.client.selector(), s.selector);
    assert!(!s.client.paused());
}

#[test]
fn verify_paths_forward_when_unpaused() {
    // Covers: RiscZeroVerifierInterface::verify and verify_integrity happy paths.
    // RiscZeroVerifierClient is panic-on-error; a successful return implies the
    // inner verifier was invoked and accepted the receipt.
    let s = setup();
    let mock = RiscZeroMockVerifierClient::new(&s.env, &s.inner);
    let image_id = BytesN::from_array(&s.env, &[0u8; 32]);
    let journal = BytesN::from_array(&s.env, &[1u8; 32]);
    let receipt = mock.mock_prove(&image_id, &journal);
    s.client.verify(&receipt.seal, &image_id, &journal);

    let receipt = Receipt {
        seal: seal_with_selector(&s.env, &s.selector),
        claim_digest: zero_digest(&s.env),
    };
    s.client.verify_integrity(&receipt);
}

#[test]
fn estop_pauses_and_blocks_subsequent_calls() {
    // Covers (in one flow):
    //   - estop() owner-happy path
    //   - paused() == true post-estop
    //   - selector() still works after estop (router-cleanup invariant)
    //   - verify() panics via #[when_not_paused]
    //   - verify_integrity() panics via #[when_not_paused]
    //   - estop() panics on already-paused (pausable::pause inner check)
    //   - estop_with_proof() panics via inner verify_integrity's #[when_not_paused]
    let s = setup();
    s.client.estop();
    assert!(s.client.paused());

    // Selector lookup must remain accessible so router admins can clean up
    // dead entries.
    assert_eq!(s.client.selector(), s.selector);

    let seal = seal_with_selector(&s.env, &s.selector);
    let image_id = BytesN::from_array(&s.env, &[0u8; 32]);
    let journal = BytesN::from_array(&s.env, &[1u8; 32]);
    assert!(s.client.try_verify(&seal, &image_id, &journal).is_err());

    let receipt = Receipt {
        seal: seal.clone(),
        claim_digest: zero_digest(&s.env),
    };
    assert!(s.client.try_verify_integrity(&receipt).is_err());
    assert!(s.client.try_estop().is_err());
    assert!(s.client.try_estop_with_proof(&receipt).is_err());
}

#[test]
#[should_panic]
fn estop_non_owner_panics() {
    // Covers: #[only_owner] auth gate on estop().
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
fn estop_with_proof_branches() {
    // Covers all three branches of estop_with_proof in one flow:
    //   1. Err InvalidProofOfExploit (non-zero claim_digest, wrapper stays unpaused).
    //   2. Inner-rejection panic with disjoint error code (wrong selector →
    //      VerifierError::InvalidSelector must surface as InvokeError, NOT as
    //      a misdecoded EmergencyStopError variant — regression for the 1-99 vs
    //      100+ error-code split).
    //   3. Ok branch (valid zero-digest receipt pauses the wrapper).
    let s = setup();

    let receipt = Receipt {
        seal: seal_with_selector(&s.env, &s.selector),
        claim_digest: BytesN::from_array(&s.env, &[1u8; 32]),
    };
    match s.client.try_estop_with_proof(&receipt) {
        Err(Ok(e)) => assert_eq!(e, EmergencyStopError::InvalidProofOfExploit),
        other => panic!("expected InvalidProofOfExploit, got {:?}", other),
    }
    assert!(!s.client.paused());

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

    let receipt = Receipt {
        seal: seal_with_selector(&s.env, &s.selector),
        claim_digest: zero_digest(&s.env),
    };
    s.client.estop_with_proof(&receipt);
    assert!(s.client.paused());
}

#[test]
fn pausable_pause_and_unpause_both_panic() {
    // Covers: Pausable::pause panics with UseEstopInstead,
    //         Pausable::unpause panics with CannotUnpause.
    // Both panics are unconditional, so a single setup is sufficient.
    let s = setup();

    match s.client.try_pause(&s.guardian) {
        Err(Ok(e)) => assert_eq!(e, EmergencyStopError::UseEstopInstead.into()),
        other => panic!("expected UseEstopInstead, got {:?}", other),
    }
    match s.client.try_unpause(&s.guardian) {
        Err(Ok(e)) => assert_eq!(e, EmergencyStopError::CannotUnpause.into()),
        other => panic!("expected CannotUnpause, got {:?}", other),
    }
}

#[test]
fn ownership_can_transfer_two_step() {
    // Covers: Ownable two-step transfer (transfer_ownership, accept_ownership)
    //         and that the new owner can drive estop after transfer.
    let s = setup();
    let new_owner = Address::generate(&s.env);

    let live_until = s.env.ledger().sequence().saturating_add(1000);
    s.client.transfer_ownership(&new_owner, &live_until);
    s.client.accept_ownership();

    assert_eq!(s.client.get_owner(), Some(new_owner.clone()));

    s.client.estop();
    assert!(s.client.paused());
}
