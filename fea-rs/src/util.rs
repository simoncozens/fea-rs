//! helpers and utilties (mostly for testing/debugging?)

pub mod debug;
mod highlighting;
mod paths;
#[cfg(any(test, feature = "diff"))]
pub mod pretty_diff;
#[cfg(any(test, feature = "test"))]
pub mod ttx;

pub use highlighting::{stringify_errors, style_for_kind};
pub use paths::rebase_path;
#[cfg(any(test, feature = "diff"))]
pub use pretty_diff::write_line_diff;

#[doc(hidden)]
pub static SPACES: &str = "                                                                                                                                                                                    ";
