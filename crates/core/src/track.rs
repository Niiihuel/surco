/// One playable item as the catalog describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    /// The catalog's stable id for the recording; also the identity used by the queue.
    pub id: String,
    pub title: String,
    pub artist: String,
    pub artist_id: Option<String>,
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub artwork_url: Option<String>,
    /// Zero when the catalog did not say.
    pub duration_ms: u64,
}
