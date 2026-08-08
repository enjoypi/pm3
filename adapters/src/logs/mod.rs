mod rotate;
mod tail;

pub use self::{
    rotate::CopyTruncateRotator,
    tail::{LogFollower, LogReadError, read_tail, tail_lines},
};
