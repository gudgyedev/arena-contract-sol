#![allow(deprecated)]
#![allow(unexpected_cfgs)]

pub mod entrypoint;
pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;

solana_program::declare_id!("AV4FTAiteCN75iq6QbuPTuh2PVL4FKwyiWJiowhhzAsQ");

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::pubkey::Pubkey;

    #[test]
    fn test_id() {
        assert_ne!(id(), Pubkey::default());
    }
}
