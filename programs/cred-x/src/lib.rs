pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("Ces2ZsycAiQy79EKb9JPcCVosr3FzvrzWEpEy9XRZif5");

#[program]
pub mod cred_x {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        initialize_protocol::handler(ctx)
    }
}
