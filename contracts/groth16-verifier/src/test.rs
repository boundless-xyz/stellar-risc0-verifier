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

extern crate std;

use soroban_sdk::Env;
use std::println;

use crate::{
    RiscZeroGroth16Verifier, RiscZeroGroth16VerifierClient, test_fixtures::prepare_inputs,
};

/// Helper to setup test environment and client
fn setup_test() -> (Env, RiscZeroGroth16VerifierClient<'static>) {
    let env = Env::default();
    let contract_id = env.register(RiscZeroGroth16Verifier, ());
    let client = RiscZeroGroth16VerifierClient::new(&env, &contract_id);
    (env, client)
}

#[test]
fn test_verify_proof() {
    let (env, client) = setup_test();
    let (seal, image_id, journal_digest) = prepare_inputs(&env);

    assert_eq!(client.verify(&seal, &image_id, &journal_digest), ());
}

// ============================================================================
// BENCHMARKS - Gas Consumption Tracking
// ============================================================================

/// Prints full budget in a formatted way
fn print_budget(env: &Env, label: &str) {
    let budget = env.cost_estimate().budget();

    println!("\n========== BENCHMARK: {} ==========", label);
    budget.print();
    println!("==========================================\n");
}

#[test]
fn bench_verify() {
    let (env, client) = setup_test();
    let (seal, image_id, journal_digest) = prepare_inputs(&env);

    // Run verification
    assert_eq!(client.verify(&seal, &image_id, &journal_digest), ());

    // Print results
    print_budget(&env, "verify()");
}

#[test]
fn bench_verify_integrity() {
    let (env, client) = setup_test();
    let (seal, image_id, journal_digest) = prepare_inputs(&env);

    // Build receipt manually
    let claim = soroban_risc0_verifier_interface::ReceiptClaim::new(&env, image_id, journal_digest);
    let receipt = soroban_risc0_verifier_interface::Receipt {
        seal,
        claim_digest: claim.digest(&env),
    };

    // Run verification
    assert_eq!(client.verify_integrity(&receipt), ());

    // Print results
    print_budget(&env, "verify_integrity()");
}

#[test]
fn bench_receipt_claim_digest() {
    let (env, _client) = setup_test();
    let (_seal, image_id, journal_digest) = prepare_inputs(&env);

    // Build claim and compute digest
    let claim = soroban_risc0_verifier_interface::ReceiptClaim::new(&env, image_id, journal_digest);
    let _digest = claim.digest(&env);

    // Print results
    print_budget(&env, "ReceiptClaim::digest()");
}
