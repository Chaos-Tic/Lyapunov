//! Lyapunov's core: cards, their storage, window backends, capture and resume.
//!
//! The `lyapunov` executable exposes all of it on the command line, with JSON
//! output for the desktop interface (`lyapunov-desktop`), which talks to it
//! through subprocesses only. Nothing here opens a network connection.

pub mod apps;
pub mod backend;
pub mod capture;
pub mod card;
pub mod integrate;
pub mod resume;
pub mod store;
pub mod time;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
