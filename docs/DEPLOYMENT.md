# Deployment Guide

> [!IMPORTANT]
> This document is for administration of the deployments of routers and verifiers on a particular Stellar chain. If you just want to verify proofs in your application see the [README](../README.md)

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

TODO