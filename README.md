# Stellar Risc0 verifier
[![Build](https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/build-and-test.yml/badge.svg)](https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/build-and-test.yml)
[![Lint](https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/linter.yml/badge.svg)](https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/linter.yml)
[![Dependencies](https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/dependency-audit.yml/badge.svg)](https://github.com/NethermindEth/stellar-risc0-verifier/actions/workflows/dependency-audit.yml)

Soroban contracts to verify execution of RISC0 zkVM programs on Stellar.

## Deployments and Version Management

> [!IMPORTANT]
> Do not deploy these contracts yourself unless you have a very specific need. Instead use the managed deployments already on Stellar.

Boundless maintains deployments of the [verifier router](./contracts/risc0-router/) as well as [individual verifiers](./contracts/groth16-verifier/) for different versions of RISC0. Ensure you are calling the public functions (`verify` or `verify_integerity`) on the router contract and not the verifiers themselves. The router ensures that your proof gets automatically routed to the correct verifier implementation and also enables the safety features that can freeze particular verifier version if a critical vulnerability is found.

The owner of the router cannot upgrade or modify existing verifiers, only add new ones for new version releases and freeze old ones if necessary. If this is unacceptable to your security model you are also free to deploy your own verifier.

For more information see https://github.com/risc0/risc0-ethereum/blob/main/contracts/version-management-design.md

## Usage

To use these from your own Soroban contract, import the `soroban-risc0-verifier-interface` crate and instantiate a `RiscZeroVerifierClient` with the address of the existing router deployment on your chain

```rust
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
```

The seal should be encoded offchain before submitting in a transaction using the `encode_seal` function from the [`soroban-risc0`](./crates/soroban-risc0/) crate

## License

Apache 2.0

## Would like to contribute?

see [Contributing](./CONTRIBUTING.md).
