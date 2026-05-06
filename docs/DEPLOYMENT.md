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
