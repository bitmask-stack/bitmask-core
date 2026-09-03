// RGB 0.11 port from bitmask-next. Clippy -D warnings stays the quality gate.
#[macro_use]
extern crate amplify;

pub mod bitcoin;
pub mod carbonado;
pub mod constants;
pub mod lightning;
pub mod nostr;
pub mod proxy;
#[cfg(not(target_arch = "wasm32"))]
pub mod regtest;

pub mod bp;
pub mod rgb;
pub mod structs;
pub mod util;
pub mod validators;
pub mod wallet;
#[cfg(target_arch = "wasm32")]
pub mod web;
