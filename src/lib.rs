//! A framework for writing network application with a non-blocking IO loop.
//! Based on the metal io library.
//!
//! Currently support TCP.
//!
//! Totally alpha.
//!

extern crate mio;
extern crate nix;
extern crate slab;

#[macro_use]
extern crate log;
extern crate env_logger;

mod rio;
mod transport;
mod interface;


pub use interface::{ServerFactory, Protocol, Reason};
pub use transport::Transport;
pub use rio::Rio;
