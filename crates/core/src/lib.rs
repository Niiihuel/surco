//! Player state shared by every front end: what a track is and what plays next.
//! Nothing here touches the network, the disk or a window, so all of it is unit-tested.

mod catalog;
pub mod lyrics;
mod queue;
mod track;

pub use catalog::{Album, AlbumRef, Artist, ArtistRef, Found};
pub use queue::{Queue, Repeat};
pub use track::Track;
