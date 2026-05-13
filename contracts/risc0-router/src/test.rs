use super::*;
use soroban_risc0_mock_verifier::{RiscZeroMockVerifier, RiscZeroMockVerifierClient};
use soroban_risc0_verifier_interface::Receipt;
use soroban_sdk::{
    Address, Bytes, BytesN, Env, IntoVal, Symbol, symbol_short, testutils::Address as _,
};

// =============================================================================
// Helper Functions
// =============================================================================

fn setup_env() -> (Env, Address, RiscZeroVerifierRouterClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(RiscZeroVerifierRouter, (admin.clone(),));
    let client = RiscZeroVerifierRouterClient::new(&env, &contract_id);

    (env, admin, client)
}

fn create_selector(env: &Env, bytes: [u8; 4]) -> BytesN<4> {
    BytesN::from_array(env, &bytes)
}

fn create_seal_with_selector(env: &Env, selector: &BytesN<4>) -> Bytes {
    let mut seal_bytes = selector.to_array().to_vec();
    // Add some dummy proof data after the selector
    seal_bytes.extend_from_slice(&[0u8; 32]);
    Bytes::from_slice(env, &seal_bytes)
}

fn create_short_seal(env: &Env) -> Bytes {
    Bytes::from_slice(env, &[0u8; 3])
}

fn setup_verifier(env: &Env) -> (BytesN<4>, Address) {
    let selector = create_selector(env, [0x01, 0x02, 0x03, 0x04]);
    let verifier_id = env.register(RiscZeroMockVerifier, (selector.clone(),));
    (selector, verifier_id)
}

fn setup_two_verifiers(
    env: &Env,
    client: &RiscZeroVerifierRouterClient<'static>,
) -> (BytesN<4>, BytesN<4>, Address, Address) {
    let selector_a = create_selector(env, [0x01, 0x02, 0x03, 0x04]);
    let selector_b = create_selector(env, [0x10, 0x20, 0x30, 0x40]);

    let verifier_a = env.register(RiscZeroMockVerifier, (selector_a.clone(),));
    let verifier_b = env.register(RiscZeroMockVerifier, (selector_b.clone(),));

    client.add_verifier(&verifier_a);
    client.add_verifier(&verifier_b);

    (selector_a, selector_b, verifier_a, verifier_b)
}

/// Helper to extract VerifierError from the nested Result type
fn unwrap_verifier_error<T: core::fmt::Debug>(
    result: Result<
        Result<T, soroban_sdk::ConversionError>,
        Result<VerifierError, soroban_sdk::InvokeError>,
    >,
) -> VerifierError {
    match result {
        Err(Ok(e)) => e,
        _ => panic!("Expected VerifierError but got {:?}", result),
    }
}

// =============================================================================
// Constructor Tests
// =============================================================================

#[test]
fn test_constructor_sets_owner() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register(RiscZeroVerifierRouter, (admin.clone(),));
    let client = RiscZeroVerifierRouterClient::new(&env, &contract_id);

    assert_eq!(client.get_owner(), Some(admin));
}

// =============================================================================
// Add Verifier Tests
// =============================================================================

#[test]
fn test_add_verifier_success() {
    let (env, _admin, client) = setup_env();

    let (selector, verifier_address) = setup_verifier(&env);

    // Non-try version - will panic on error
    client.add_verifier(&verifier_address);

    // Verify it was added
    let result = client.get_verifier_by_selector(&selector);
    assert_eq!(result, verifier_address);
}

#[test]
fn test_add_verifier_selector_in_use() {
    let (env, _admin, client) = setup_env();

    let (_, verifier1) = setup_verifier(&env);
    let (_, verifier2) = setup_verifier(&env);

    // First add should succeed
    client.add_verifier(&verifier1);

    // Second add with same selector should fail - use try_ to capture error
    let result = client.try_add_verifier(&verifier2);
    assert_eq!(unwrap_verifier_error(result), VerifierError::SelectorInUse);
}

#[test]
fn test_add_verifier_tombstone_selector() {
    let (env, _admin, client) = setup_env();

    let (selector, verifier) = setup_verifier(&env);

    // Manually set a tombstone entry
    env.as_contract(&client.address, || {
        env.storage().persistent().set(
            &DataKey::Verifier(selector.clone()),
            &VerifierEntry::Tombstone,
        );
    });

    // Adding to tombstoned selector should fail - use try_ to capture error
    let result = client.try_add_verifier(&verifier);
    assert_eq!(
        unwrap_verifier_error(result),
        VerifierError::SelectorRemoved
    );
}

// =============================================================================
// Raw Verifier Entry Tests
// =============================================================================

#[test]
fn test_verifiers_getter_returns_raw_entry() {
    let (env, _admin, client) = setup_env();

    let (selector, verifier_address) = setup_verifier(&env);

    // Unset selector should return None.
    assert_eq!(client.verifiers(&selector), None);

    client.add_verifier(&verifier_address);

    assert_eq!(
        client.verifiers(&selector),
        Some(VerifierEntry::Active(verifier_address))
    );

    client.remove_verifier(&selector);

    assert_eq!(client.verifiers(&selector), Some(VerifierEntry::Tombstone));
}

// =============================================================================
// Remove Verifier Tests
// =============================================================================

#[test]
fn test_remove_verifier_marks_tombstone() {
    let (env, _admin, client) = setup_env();

    let (selector, verifier_address) = setup_verifier(&env);

    client.add_verifier(&verifier_address);
    client.remove_verifier(&selector);

    let result = client.try_get_verifier_by_selector(&selector);
    assert_eq!(
        unwrap_verifier_error(result),
        VerifierError::SelectorRemoved
    );

    let (_, new_verifier) = setup_verifier(&env);
    let result = client.try_add_verifier(&new_verifier);
    assert_eq!(
        unwrap_verifier_error(result),
        VerifierError::SelectorRemoved
    );
}

#[test]
fn test_remove_verifier_unknown_selector() {
    let (env, _admin, client) = setup_env();

    let selector = create_selector(&env, [0xAA, 0xBB, 0xCC, 0xDD]);
    let result = client.try_remove_verifier(&selector);
    assert_eq!(
        unwrap_verifier_error(result),
        VerifierError::SelectorUnknown
    );
}

#[test]
fn test_removed_selector_blocks_verify() {
    let (env, _admin, client) = setup_env();

    let (_selector_a, selector_b, verifier_a, _verifier_b) = setup_two_verifiers(&env, &client);
    let mock_a = RiscZeroMockVerifierClient::new(&env, &verifier_a);
    client.remove_verifier(&selector_b);

    let image_id = BytesN::from_array(&env, &[0u8; 32]);
    let journal_digest = BytesN::from_array(&env, &[1u8; 32]);

    // Selector A still routes — success implies the router dispatched to A and
    // the mock accepted the receipt (a wrong-selector dispatch would have
    // returned `InvalidSelector`).
    let receipt_a = mock_a.mock_prove(&image_id, &journal_digest);
    client.verify(&receipt_a.seal, &image_id, &journal_digest);

    // Selector B is tombstoned — the router rejects before any inner
    // dispatch, so the seal body doesn't have to be a valid mock receipt.
    let seal_b = create_seal_with_selector(&env, &selector_b);
    let result = client.try_verify(&seal_b, &image_id, &journal_digest);
    assert_eq!(
        unwrap_verifier_error(result),
        VerifierError::SelectorRemoved
    );
}

#[test]
fn test_removed_selector_blocks_verify_integrity() {
    let (env, _admin, client) = setup_env();

    let (selector_a, selector_b, _verifier_a, _verifier_b) = setup_two_verifiers(&env, &client);
    client.remove_verifier(&selector_b);

    // `selector || [0u8; 32]` paired with claim_digest = [0u8; 32] satisfies
    // the mock's `keccak(seal[4..]) == keccak(claim_digest)` check, so the
    // router-A path completes end-to-end.
    let receipt_a = Receipt {
        seal: create_seal_with_selector(&env, &selector_a),
        claim_digest: BytesN::from_array(&env, &[0u8; 32]),
    };
    client.verify_integrity(&receipt_a);

    // Selector B is tombstoned — router rejection short-circuits before any
    // inner dispatch.
    let receipt_b = Receipt {
        seal: create_seal_with_selector(&env, &selector_b),
        claim_digest: BytesN::from_array(&env, &[0u8; 32]),
    };
    let result = client.try_verify_integrity(&receipt_b);
    assert_eq!(
        unwrap_verifier_error(result),
        VerifierError::SelectorRemoved
    );
}

// =============================================================================
// Verification Routing Tests
// =============================================================================

#[test]
fn test_verify_routes_to_correct_verifier() {
    let (env, _admin, client) = setup_env();

    let (_selector, mock_verifier_id) = setup_verifier(&env);
    let mock_client = RiscZeroMockVerifierClient::new(&env, &mock_verifier_id);
    client.add_verifier(&mock_verifier_id);

    let image_id = BytesN::from_array(&env, &[0u8; 32]);
    let journal_digest = BytesN::from_array(&env, &[1u8; 32]);
    let receipt = mock_client.mock_prove(&image_id, &journal_digest);

    // Successful return implies the router dispatched to the mock and the mock
    // accepted the receipt — a misroute would have returned `InvalidSelector`.
    let _: () = env.invoke_contract(
        &client.address,
        &symbol_short!("verify"),
        (receipt.seal, image_id, journal_digest).into_val(&env),
    );
}

#[test]
fn test_verify_routes_to_multiple_verifiers() {
    let (env, _admin, client) = setup_env();

    let (_selector_a, _selector_b, verifier_a, verifier_b) = setup_two_verifiers(&env, &client);
    let mock_a = RiscZeroMockVerifierClient::new(&env, &verifier_a);
    let mock_b = RiscZeroMockVerifierClient::new(&env, &verifier_b);

    let image_id = BytesN::from_array(&env, &[0u8; 32]);
    let journal_digest = BytesN::from_array(&env, &[1u8; 32]);

    // Each mock only accepts seals prefixed with its own selector, so
    // successful verification implies the router routed to the right one.
    let receipt_a = mock_a.mock_prove(&image_id, &journal_digest);
    client.verify(&receipt_a.seal, &image_id, &journal_digest);

    let receipt_b = mock_b.mock_prove(&image_id, &journal_digest);
    client.verify(&receipt_b.seal, &image_id, &journal_digest);
}

#[test]
fn test_verify_returns_verifier_error_on_failure() {
    let (env, _admin, client) = setup_env();

    let (selector, verifier_id) = setup_verifier(&env);
    client.add_verifier(&verifier_id);

    // `selector || [0u8; 32]` paired with a non-zero claim_digest derived from
    // (image_id, journal) trips the mock's `keccak(seal[4..]) ==
    // keccak(claim_digest)` check, surfacing as `InvalidProof` through the
    // router.
    let seal = create_seal_with_selector(&env, &selector);
    let image_id = BytesN::from_array(&env, &[0u8; 32]);
    let journal_digest = BytesN::from_array(&env, &[1u8; 32]);

    let result = client.try_verify(&seal, &image_id, &journal_digest);
    assert_eq!(unwrap_verifier_error(result), VerifierError::InvalidProof);
}

#[test]
fn test_verify_integrity_routes_to_correct_verifier() {
    let (env, _admin, client) = setup_env();

    let (selector, verifier_id) = setup_verifier(&env);
    client.add_verifier(&verifier_id);

    // `selector || [0u8; 32]` paired with claim_digest = [0u8; 32] satisfies
    // the mock's keccak check, so a successful return implies the router
    // dispatched to the mock for this selector.
    let receipt = Receipt {
        seal: create_seal_with_selector(&env, &selector),
        claim_digest: BytesN::from_array(&env, &[0u8; 32]),
    };

    let _: () = env.invoke_contract(
        &client.address,
        &Symbol::new(&env, "verify_integrity"),
        (receipt,).into_val(&env),
    );
}

#[test]
fn test_verify_integrity_routes_to_multiple_verifiers() {
    let (env, _admin, client) = setup_env();

    let (selector_a, selector_b, _verifier_a, _verifier_b) = setup_two_verifiers(&env, &client);

    // claim_digest = [0u8; 32] paired with `selector || [0u8; 32]` matches the
    // mock's keccak check; routing to the wrong mock would surface as
    // `InvalidSelector`.
    let claim_digest = BytesN::from_array(&env, &[0u8; 32]);

    let receipt_a = Receipt {
        seal: create_seal_with_selector(&env, &selector_a),
        claim_digest: claim_digest.clone(),
    };
    client.verify_integrity(&receipt_a);

    let receipt_b = Receipt {
        seal: create_seal_with_selector(&env, &selector_b),
        claim_digest,
    };
    client.verify_integrity(&receipt_b);
}

#[test]
fn test_verify_integrity_returns_verifier_error_on_failure() {
    let (env, _admin, client) = setup_env();

    let (selector, verifier_id) = setup_verifier(&env);
    client.add_verifier(&verifier_id);

    // seal[4..] = [0u8; 32] but claim_digest = [1u8; 32] — keccak hashes
    // differ, so the mock returns `InvalidProof` and the router surfaces it.
    let receipt = Receipt {
        seal: create_seal_with_selector(&env, &selector),
        claim_digest: BytesN::from_array(&env, &[1u8; 32]),
    };

    let result = client.try_verify_integrity(&receipt);
    assert_eq!(unwrap_verifier_error(result), VerifierError::InvalidProof);
}

#[test]
#[should_panic]
fn test_verify_panics_on_unknown_selector() {
    let (env, _admin, client) = setup_env();

    let selector = create_selector(&env, [0x01, 0x02, 0x03, 0x04]);
    let seal = create_seal_with_selector(&env, &selector);
    let image_id = BytesN::from_array(&env, &[0u8; 32]);
    let journal_digest = BytesN::from_array(&env, &[1u8; 32]);

    // This should panic because no verifier is registered for this selector
    let _: () = env.invoke_contract(
        &client.address,
        &symbol_short!("verify"),
        (seal, image_id, journal_digest).into_val(&env),
    );
}

#[test]
fn test_verify_malformed_seal() {
    let (env, _admin, client) = setup_env();

    let seal = create_short_seal(&env);
    let image_id = BytesN::from_array(&env, &[0u8; 32]);
    let journal_digest = BytesN::from_array(&env, &[1u8; 32]);

    let result = client.try_verify(&seal, &image_id, &journal_digest);
    assert_eq!(unwrap_verifier_error(result), VerifierError::MalformedSeal);
}

#[test]
fn test_verify_integrity_malformed_seal() {
    let (env, _admin, client) = setup_env();

    let seal = create_short_seal(&env);
    let receipt = Receipt {
        seal,
        claim_digest: BytesN::from_array(&env, &[0u8; 32]),
    };

    let result = client.try_verify_integrity(&receipt);
    assert_eq!(unwrap_verifier_error(result), VerifierError::MalformedSeal);
}

// =============================================================================
// Admin Authorization Tests
// =============================================================================

#[test]
#[should_panic]
fn test_add_verifier_requires_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(RiscZeroVerifierRouter, (admin.clone(),));
    let client = RiscZeroVerifierRouterClient::new(&env, &contract_id);
    env.set_auths(&[]);

    let verifier = Address::generate(&env);

    // Should trap on admin.require_auth().
    client.add_verifier(&verifier);
}

#[test]
#[should_panic]
fn test_remove_verifier_requires_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(RiscZeroVerifierRouter, (admin.clone(),));
    let client = RiscZeroVerifierRouterClient::new(&env, &contract_id);
    env.set_auths(&[]);

    let selector = create_selector(&env, [0x01, 0x02, 0x03, 0x04]);
    let verifier = Address::generate(&env);

    env.as_contract(&client.address, || {
        env.storage().persistent().set(
            &DataKey::Verifier(selector.clone()),
            &VerifierEntry::Active(verifier),
        );
    });

    // Should trap on admin.require_auth().
    client.remove_verifier(&selector);
}
