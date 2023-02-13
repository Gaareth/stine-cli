#![warn(
clippy::all,
clippy::cargo,
clippy::nursery,
// clippy::pedantic
)]

#![allow(clippy::use_self)]

extern crate core;

mod stine;

mod types;
mod parse;
mod utils;

pub use stine::{*};
pub use types::document::{*};
pub use types::event::{*};
pub use types::language::{*};
pub use types::period::{*};
pub use types::semester::{*};


// TODO: cache data wrapper including language maybe time
// improve submodule attribute parsing maybe similar to module
