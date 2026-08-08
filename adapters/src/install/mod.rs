mod error;
mod layout;
mod probe;
mod store;

pub use self::{
    error::InstallError,
    layout::{backup_name, backup_root, destination_of, parse_version_output},
    probe::binary_version,
    store::{back_up, replace_binary},
};
