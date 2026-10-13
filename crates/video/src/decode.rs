//! H.264 pictures into frames a window can show.

use openh264::formats::YUVSource;

/// One picture, as rows of RGBA.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

pub struct Decoder(openh264::decoder::Decoder);

impl Decoder {
    /// A decoder that has been given a rendition's parameter sets.
    pub fn new(parameters: &[u8]) -> Result<Self, String> {
        // Pictures that lean on later ones come out late: flushing after each, the default,
        // throws away what the decoder was holding for them.
        let config = openh264::decoder::DecoderConfig::new().flush_after_decode(openh264::decoder::Flush::NoFlush);
        let mut decoder = openh264::decoder::Decoder::with_api_config(openh264::OpenH264API::from_source(), config).map_err(|error| error.to_string())?;
        decoder.decode(parameters).map_err(|error| error.to_string())?;
        Ok(Self(decoder))
    }

    /// Takes the next picture in. A frame comes out when the decoder has one to show, which
    /// is a few pictures behind where pictures lean on later ones.
    pub fn decode(&mut self, picture: &[u8]) -> Result<Option<Frame>, String> {
        let Some(decoded) = self.0.decode(picture).map_err(|error| error.to_string())? else { return Ok(None) };
        let (width, height) = decoded.dimensions();
        let mut rgba = vec![0; width * height * 4];
        decoded.write_rgba8(&mut rgba);
        Ok(Some(Frame { width, height, rgba }))
    }
}
