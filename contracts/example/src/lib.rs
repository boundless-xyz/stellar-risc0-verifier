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

use soroban_sdk::{Address, Bytes, BytesN, Env, Symbol, contract, contractimpl, symbol_short};

use soroban_risc0_verifier_interface::RiscZeroVerifierClient;

#[contract]
pub struct ExampleContract;

const VERIFIER_ROUTER: Symbol = symbol_short!("VF_RTR");
const IMAGE_ID: Symbol = symbol_short!("IMG_ID");

#[contractimpl]
impl ExampleContract {
    pub fn __constructor(env: Env, router_address: Address, image_id: &BytesN<32>) {
        // store the router address
        env.storage()
            .instance()
            .set(&VERIFIER_ROUTER, &router_address);
        // store the image id
        env.storage().instance().set(&IMAGE_ID, image_id);
    }

    pub fn verify_proof(env: Env, seal: &Bytes, journal: &Bytes) -> bool {
        let router_address = env.storage().instance().get(&VERIFIER_ROUTER).unwrap();
        let image_id = env.storage().instance().get(&IMAGE_ID).unwrap();
        let journal_digest = env.crypto().sha256(journal).to_bytes();

        // call to the router to verify the proof; will return an error if verification
        // fails
        let client = RiscZeroVerifierClient::new(&env, &router_address);
        client.verify(seal, &image_id, &journal_digest);

        true
    }
}
