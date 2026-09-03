# BitMask Core
Core functionality for the BitMask wallet - <https://bitmask.app>

**BitMask** is a Bitcoin-only web wallet and browser extension for accessing decentralized web applications on the Bitcoin timechain. It is designed to support UTXO-based smart contracting protocols such as [RGB](https://rgb.tech), in addition to Lightning payments.

[![Build status](https://img.shields.io/github/actions/workflow/status/diba-io/bitmask-core/rust.yaml?branch=development&style=flat-square)](https://github.com/diba-io/bitmask-core/actions/workflows/rust.yaml)
[![Crates.io](https://img.shields.io/crates/v/bitmask-core?style=flat-square)](https://docs.rs/bitmask-core/latest/bitmask-core/)
[![npm: bitmask-core](https://img.shields.io/npm/v/bitmask-core?style=flat-square)](https://www.npmjs.com/package/bitmask-core)
[![License: MIT+APACHE](https://img.shields.io/crates/l/bitmask-core?style=flat-square)](https://mit-license.org)
[![Telegram](https://img.shields.io/badge/telegram-invite-blue?style=flat-square)](https://t.me/+eQk5aQ5--iUxYzVk)

## Uses

- [bdk_wallet](https://github.com/bitcoindevkit/bdk_wallet) 3.1 - Bitcoin Dev Kit (BDK 1.x; names testnet4)
- [rgb-ops](https://github.com/rgb-protocol/rgb-ops) 0.11.1-rc.11 - RGB Protocol on Bitcoin ([docs.rgb.info](https://docs.rgb.info/), accessed: 2026-08-26)
- [wasm-pack](https://github.com/rustwasm/wasm-pack) - WebAssembly
- [lndhubx](https://lndhubx.kollider.xyz) - Custodial Lightning
- [nostr](https://crates.io/crates/nostr/0.44.8) - Nostr 0.44.8 key parse, bech32, and x-only public keys
- [carbonado](https://crates.io/crates/carbonado/0.7.0) 0.7.0 - Carbonado e2ee decentralized storage (v2 AES-CTR+HMAC; no ECIES v1)

## Build

This should work with either wasm-pack, [trunk](https://github.com/thedodd/trunk), or x86-64.

Some environment variables may be needed in order to compile on macos-aarch64, for more, [see this](https://github.com/sapio-lang/sapio/issues/146#issuecomment-960659800).

If there are issues compiling, be sure to check you're compiling with the latest Rust version.

To build this as a NodeJS module, use: `wasm-pack build --release --target bundler`

## Test

1. Lint against wasm32: `cargo clippy --target wasm32-unknown-unknown`
2. Run tests in browser: `TEST_WALLET_SEED="replace with a 12 word mnemonic for a wallet containing testnet sats" wasm-pack test --headless --chrome`

## Run

To run the bitmaskd node with REST server, either for testing the web wallet, or simply for increased privacy:

`cargo install --features=server --path .`

Then run `bitmaskd`.

## Development

Parts of this application are built with conditional compilation statements for wasm32 support. This is a helpful command for checking linting and correctness while also developing on desktop platforms:

`cargo clippy --target wasm32-unknown-unknown --no-default-features --release`

## Release

Upon a new release, follow these steps:

1. Run `cargo update` to update to latest deps.
1. Run `cargo +nightly udeps` to see if there are any unused dependencies.

## Docker

For running bitmask-core tests in Regtest Mode, please follow the steps below:

### Initial Setup

1. Build bitcoin node + electrum: `docker-compose build`.
2. Up and running Docker containers: `docker-compose up -d node1 bitmaskd`.
3. Load the command line: `source .commands`
4. Download and install BDK cli: `cargo install bdk-cli`. We will use BDK to generate the mnemonic.
5. Generate a new mnemonic: `bdk-cli generate`.
6. Create an environment variable called **TEST_WALLET_SEED** with mnemonic generated in the **step 5** (only wasm32).
7. Run the test to get main address for bitcoin and rgb: `cargo test --test wallet -- create_wallet --exact`.
8. Load your wallet in the bitcoin node: `node1 loadwallet default`.
9. Generate new first 500 blocks: `node1 -generate 500`.
10. Send some coins to the main wallet address: `node1 sendtoaddress {MAIN_VAULT_ADDRESS} 10`. Change `{MAIN_VAULT_ADDRESS}` with the address generated in the **step 7**.
11. Send some coins to the rgb wallet address: `node1 sendtoaddress {RGB_VAULT_ADDRESS} 10`. Change `{RGB_VAULT_ADDRESS}` with the address generated in the **step 7**.
12. Mine a new block: `node1 -generate 1`
13. Run the test to check the balance: `cargo test --test wallet -- get_wallet_balance --exact`.

### Running the tests

Running the tests: `cargo test --test-threads 1`

### Troubleshooting

#### **1. After restarting the container**

**A.The bitcoin node does not work?**

Check if your wallet is loaded. For that, run the command `node1 loadwallet default`.

**B.The electrs node does not work?**

To stop the electrs freeze, run `node1 -generate`.
