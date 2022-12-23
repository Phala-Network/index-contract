use super::context::Context;

pub trait Runner {
    fn run(&self, context: &Context) -> Result<(), &'static str>;
    fn check(&self, nonce: u64) -> bool;
}
