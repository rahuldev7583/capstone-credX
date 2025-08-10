#![allow(deprecated)]
use anchor_lang::prelude::*;
pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("Ces2ZsycAiQy79EKb9JPcCVosr3FzvrzWEpEy9XRZif5");

#[program]
pub mod cred_x {
    use super::*;

}
