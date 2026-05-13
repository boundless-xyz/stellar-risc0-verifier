# Deployment Guide

> [!IMPORTANT]
> This document is for administration of the deployments of routers and verifiers on a particular Stellar chain. If you just want to verify proofs in your application see the [README](../README.md)

> [!NOTE]
> This guide targets stellar-cli 26.0.0. Commands may change between versions

## Requirements

Most commands in this guide use the `stellar` CLI tool https://developers.stellar.org/docs/tools/cli/install-cli

This will also need an account (called "deployer" in this guide) which you can create with 

```shell
stellar keys generate deployer
stellar keys generate owner
```

set the network to deploy to with ("testnet" in this example)

```shell
stellar network use testnet
```

and if you are deploying to a testnet, fund the account with
```shell
stellar keys fund deployer
stellar keys fund owner
```

otherwise send it some XLM for gas. Estimate around 0.002 XLM per contract deployment

## Deploying the router

- This only needs to be done once per chain, or if you want to deploy a new router with a different owner
- Otherwise skip this step and use the address of the router in place of `soroban-risc0-router` as the `--id` param below

```shell
stellar contract deploy --package soroban-risc0-router --optimize --source-account deployer -- --owner owner
```

## Deploying and linking a verifier

- Do this when a new version of RISC0 is released that requires a change of control roots. Usually a major or minor version bump
- Ensure the version of the `risc0-circuit-recursion` crate in the build dependencies of [../contracts/groth16-verifier/Cargo.toml](../contracts/groth16-verifier/Cargo.toml) is set to the version matching the version of RISC0 you want to deploy a verifier for. The control roots are automatically imported from this crate and baked into the contract which determines the selector.

```shell
stellar contract deploy --package soroban-risc0-groth16-verifier --optimize --source-account deployer
```

Then link the verifier to the router with

```shell
stellar contract invoke --id soroban-risc0-router --source-account owner -- add_verifier --verifier soroban-risc0-groth16-verifier
```

## Deploying with an emergency stop wrapper

The [emergency-stop contract](../contracts/emergency-stop) is an optional wrapper that sits between the router and a verifier.
While unpaused it forwards `verify` and `verify_integrity` to the inner verifier; once stopped it permanently rejects them.

The wrapper cannot be restarted. Recovery is to deploy a fresh wrapper around the existing inner verifier (which is unaffected by the stop) and link the new wrapper to the router.

A wrapper can be stopped two ways:

- The **guardian** account set at construction can call `estop` at any time.
- Anyone can call `estop_with_proof` with a valid receipt for the all-zeroes claim digest.

Such a receipt can only exist if the proof system is critically broken, so this acts as a permissionless circuit-breaker.

Production deployments should wrap every verifier.

> [!IMPORTANT]
> The multisig patterns in [More Complex Ownership](#more-complex-ownership) apply equally to the guardian account.

### Deploying the wrapper

You will need a `guardian` account in addition to `deployer` and `owner`:

```shell
stellar keys generate guardian
stellar keys fund guardian   # testnet only
```

First, deploy the inner verifier as in [Deploying and linking a verifier](#deploying-and-linking-a-verifier)

Deploy a wrapper around the inner verifier with:

```shell
stellar contract deploy --package soroban-risc0-emergency-stop --optimize --source-account deployer -- --verifier soroban-risc0-groth16-verifier --guardian guardian
```

Link the wrapper, not the bare verifier, to the router.
The wrapper's `selector()` forwards from the inner verifier, so the router routes the same selector as a direct registration would:

```shell
stellar contract invoke --id soroban-risc0-router --source-account owner -- add_verifier --verifier soroban-risc0-emergency-stop
```

### Stopping and replacing a wrapper

Guardian-driven stop:

```shell
stellar contract invoke --id soroban-risc0-emergency-stop --source-account guardian -- estop
```

The permissionless circuit-breaker call is `estop_with_proof --receipt <json-encoded Receipt with all-zero claim_digest>`; see the contract source for the exact Receipt shape.

After stopping (by either path), the router owner should tombstone the wrapper's selector so subsequent verifications fail at the router rather than being dispatched to the dead wrapper.
The wrapper's `selector()` accessor still works while paused, by design:

```shell
# Read the selector from the now-paused wrapper
stellar contract invoke --id soroban-risc0-emergency-stop -- selector

# Tombstone it on the router (substitute the value from above)
stellar contract invoke --id soroban-risc0-router --source-account owner -- remove_verifier --selector <selector>
```

A new wrapper can then be deployed and linked using the same steps above.
The inner verifier is unaffected by the stop. Point the new wrapper at the existing verifier, or at a freshly deployed inner verifier if the underlying issue requires replacing that too (e.g., a successful `estop_with_proof` demonstrates a proof-system break, so any wrapper around the same inner would be stoppable the same way). However, since the wrapper's `selector()` forwards that of the inner verifier, it cannot be used to replace an existing entry in a router.

## More Complex Ownership

As described above the router is owned by the locally generated account `owner`. This is probably not what you want for a real deployment. Stellar natively supports threshold multi-sig and you can quite easily configure the owner to require signatures from multiple other accounts. For example to set up a 2-of-3 multisig as the owner you can run

```shell
# Add signer 1
stellar tx new set-options \
  --source-account owner \
  --signer G...AAA --signer-weight 1

# Add signer 2
stellar tx new set-options \
  --source-account owner \
  --signer G...BBB --signer-weight 1

# Add signer 3
stellar tx new set-options \
  --source-account owner \
  --signer G...CCC --signer-weight 1

# Raise thresholds and drop master weight 
# After this, owner account is fully multisig and signatures from the original key are obsolete
stellar tx new set-options \
  --source-account owner \
  --low-threshold 2 --med-threshold 2 --high-threshold 2 \
  --master-weight 0 
```

Important things to note:

- The final step `--master-weight 0` sets the signing weight from owner to be zero. They do not contribute anything by signing. After this you will require multi-sigs to add more signers.
- adjust the `*-threshold` values accordingly for setting up a larger number of keys

### Using Lobstr Vault

To coordinate signing between multiple parties you can use [Lobstr Vault](https://vault.lobstr.co/)

To set up:

1. Have all signers setup an account in Lobstr Vault and obtain their public G-addresses

2. Follow the above instructions but before setting the thresholds make the following call:

Be sure to do this before you set the `--master-weight 0`

```shell
stellar tx new set-options \
  --source-account owner \
  --signer GA2T6GR7VXXXBETTERSAFETHANSORRYXXXPROTECTEDBYLOBSTRVAULT --signer-weight 0
```

This adds the Lobstr Vault marker key as a signer. It does not participate in signing, it is just used as an on-chain marker for discovery by the Lobstr Vault backend service.

3. Obtain a multi-sig to make a contract call (in this case adding a new verifier to the router)

- Change `soroban-risc0-groth16-verifier` to the address or ID of the verifier you wish to add
- This writes the raw tx to a file called `tx.xdr`

```shell
stellar contract invoke \
    --id soroban-risc0-router \
    --source-account owner \
    --build-only \
    -- \
    add_verifier \
    --verifier soroban-risc0-groth16-verifier > tx.xdr
```

3. Send this to the Lobstr API for signing

```shell
curl 'https://vault.lobstr.co/api/transactions/' \
  -H 'Content-Type: application/json' \
  -d "{\"xdr\": \"$(cat tx.xdr)\"}"
```

This will trigger a notification in the app for all registered required signers to approve.  Once the threshold is met the transaction will be automatically sent to the network.
