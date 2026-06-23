//! Command handler modules for the Microscope Memory CLI.
//! Each module contains the handler function(s) for a logical group of commands.
//! Functions in these modules access `crate::open_reader()` to get the reader,
//! and `crate::timestamp_to_str()` for timestamp formatting.

pub mod bench;
pub mod cognitive;
pub mod federated;
pub mod init;
pub mod recall;
pub mod search;
pub mod verify;
pub mod viz;
