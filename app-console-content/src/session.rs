use std::time::Instant;

pub use system_gateway_mba::user::{Tenant, User};

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Idle,
    Proofread,
    Running,
    Results,
    Draft,
}

pub struct ProofSession {
    pub user: User,
    pub mode: Mode,
    pub buffer: String,
    pub started: Instant,
}

impl ProofSession {
    pub fn new(user: User) -> Self {
        Self {
            user,
            mode: Mode::Idle,
            buffer: String::new(),
            started: Instant::now(),
        }
    }
}
