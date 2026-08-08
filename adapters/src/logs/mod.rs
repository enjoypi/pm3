mod clear;
mod rotate;
mod tail;

pub use self::{
    clear::{LogClearError, clear_log},
    rotate::CopyTruncateRotator,
    tail::{LogFollower, LogReadError, read_tail, tail_lines},
};
