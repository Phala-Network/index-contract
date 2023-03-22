#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod assets;
pub mod constants;
pub mod executors;
pub mod graph;
pub mod prelude;
pub mod traits;
mod transactors;
pub mod tx;
pub mod utils;

#[derive(Clone)]
pub enum AccountType {
    Account20,
    Account32,
}
