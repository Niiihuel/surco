use crate::Track;

/// An artist as a list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtistRef {
    pub id: String,
    pub name: String,
    /// The catalog's own wording, such as "2.1M subscribers".
    pub subscribers: Option<String>,
    pub artwork_url: Option<String>,
}

/// An album, a single or an EP as a list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlbumRef {
    pub id: String,
    pub title: String,
    /// Absent where the list already belongs to one artist.
    pub artist: Option<String>,
    /// Empty when the catalog did not say.
    pub year: String,
    pub artwork_url: Option<String>,
}

/// What a search found, each kind in the catalog's own order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub artists: Vec<ArtistRef>,
    pub albums: Vec<AlbumRef>,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub subscribers: Option<String>,
    pub artwork_url: Option<String>,
    pub songs: Vec<Track>,
    pub albums: Vec<AlbumRef>,
    pub singles: Vec<AlbumRef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub artist_id: Option<String>,
    pub year: String,
    pub artwork_url: Option<String>,
    pub tracks: Vec<Track>,
}
