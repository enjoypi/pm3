use std::future::Future;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Liveness {
    Alive(String),
    Gone,
    Unreadable,
}

pub trait ProcessProbe: Send + Sync {
    fn identity(&self, pid: u32) -> impl Future<Output = Liveness> + Send;
}

impl Liveness {
    #[must_use]
    pub fn into_token(self) -> Option<String> {
        match self {
            Self::Alive(token) => Some(token),
            Self::Gone | Self::Unreadable => None,
        }
    }
}
