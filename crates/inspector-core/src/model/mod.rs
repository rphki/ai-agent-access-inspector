//! Agent-independent data model. See `docs/findings-model.md`.

mod diagnostic;
mod finding;
mod message;
mod scan;

pub use diagnostic::*;
pub use finding::*;
pub use message::*;
pub use scan::*;
