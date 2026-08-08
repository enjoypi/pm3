mod error;
mod layout;
mod store;

pub use self::{
    error::InstallError,
    layout::{backup_root, backup_stamp, destination_of},
    store::{back_up, replace_binary},
};
