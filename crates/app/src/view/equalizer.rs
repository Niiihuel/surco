//! The equaliser's parts: the curve its filters draw, a slider standing for each band, the
//! ready-made curves and the preamp. A popover in a window; a page of cards on a phone.

use super::*;
use crate::state::{equalizer_preset, EQUALIZER_PRESETS};
use limn::{ChartSeries, LineChart};
use surco_audio::equalizer::response_db;

/// The rate the curve is worked out at: the shape is the same at any the audio plays at.
const SAMPLE_RATE: f32 = 48_000.0;
/// Points of the curve between one band's centre and the next, and below the first, from 20 Hz.
const PER_BAND: usize = 6;
const LEAD: usize = 4;
/// From the last band to 20 kHz, a third of an octave.
const TAIL: usize = 2;
/// Every band's stop, from -12 to +12 dB a decibel apart.
const STOPS: usize = EQUALIZER_DB as usize * 2 + 1;

/// The curve's frequencies, evenly spaced in pitch from 20 Hz to 20 kHz, each band's centre among them.
fn frequencies() -> Vec<f32> {
    let mut spans = vec![(20.0, BAND_HZ[0], LEAD)];
    spans.extend(BAND_HZ.windows(2).map(|pair| (pair[0], pair[1], PER_BAND)));
    spans.push((BAND_HZ[BAND_HZ.len() - 1], 20_000.0, TAIL));
    let mut points: Vec<f32> = spans
        .into_iter()
        .flat_map(|(from, to, steps)| (0..steps).map(move |step| from * (to / from).powf(step as f32 / steps as f32)))
        .collect();
    points.push(20_000.0);
    points
}

/// Where a band's centre is among the curve's points.
fn band_point(band: usize) -> usize {
    LEAD + band * PER_BAND
}

/// "31", "1K": a band as its column names it.
fn band_name(hz: f32) -> String {
    if hz >= 1000.0 { format!("{:.0}K", hz / 1000.0) } else { format!("{hz:.0}") }
}

fn hertz(hz: f32) -> String {
    let kilo = hz / 1000.0;
    match kilo {
        _ if kilo < 1.0 => format!("{hz:.0} Hz"),
        _ if kilo.fract() == 0.0 => format!("{kilo:.0} kHz"),
        _ => format!("{kilo:.1} kHz"),
    }
}

pub(super) fn decibels(gain: f32) -> String {
    if gain == 0.0 { "0 dB".into() } else { format!("{gain:+.0} dB") }
}

/// What the bands are set to, as Settings names it.
pub(super) fn summary(state: &State) -> &'static str {
    equalizer_preset(state.equalizer).map_or("Personalizado", |(_, name)| name)
}

fn fraction(gain: f32) -> f32 {
    (gain / EQUALIZER_DB + 1.0) / 2.0
}

/// The response of the very filters the audio goes through, white on `ground`, from -12 to
/// +12 dB, with the band last moved marked.
pub(super) fn curve(state: &State, height: f32, ground: Color) -> Element {
    let palette = ComponentPalette { surface: ground, border: Color::rgb(110, 110, 110), ..SCHEME.palette() };
    let points = frequencies();
    let limit = f64::from(EQUALIZER_DB);
    let gains = points.iter().map(|hz| f64::from(response_db(&state.equalizer, *hz, SAMPLE_RATE)).clamp(-limit, limit));
    let mut chart = LineChart::new("equalizer-curve", points.iter().map(|hz| hertz(*hz)), vec![ChartSeries::new("Ganancia", gains).color(Color::WHITE)], palette)
        .height(height)
        .area(true)
        .grid(false)
        .point_axis(false)
        .dashed_zero(true)
        .range(-limit, limit);
    if let Some(band) = state.equalizer_band {
        chart = chart.mark(band_point(band));
    }
    chart.render()
}

/// A slider standing for each band, its gain over it and its frequency under it; the band
/// last moved has both in white.
pub(super) fn bands(state: &State, height: f32) -> Element {
    let palette = SCHEME.on_raised();
    let labels: Vec<String> = (0..STOPS).map(|stop| decibels(stop as f32 - EQUALIZER_DB)).collect();
    let columns = BAND_HZ
        .iter()
        .zip(state.equalizer.bands_db)
        .enumerate()
        .map(|(band, (hz, gain))| {
            let ink = if state.equalizer_band == Some(band) { Color::WHITE } else { SCHEME.muted };
            let small = |text: String| Text::new(text, palette).font_size(11.0).ink(ink).render().centered_x();
            let slider = Slider::new(format!("equalizer:{band}"), fraction(gain), palette)
                .vertical()
                .origin(0.5)
                .steps(STOPS)
                .mark_labels(labels.clone())
                .width(height)
                .ink(Color::WHITE)
                .accessibility_label(hertz(*hz))
                .render();
            let gain = if gain == 0.0 { "0".into() } else { format!("{gain:+.0}") };
            Element::column(vec![small(gain), slider, small(band_name(*hz))]).gap(6.0).flex(1.0)
        })
        .collect();
    Element::row(columns)
}

/// The ready-made curves as capsules, the one the bands are set to in white; once a band is
/// moved off them, a passive "Personalizado" in white before them, where it is seen.
pub(super) fn presets(state: &State, height: f32) -> Element {
    let palette = SCHEME.palette();
    let chosen = equalizer_preset(state.equalizer).map(|(key, _)| key);
    let mut pills: Vec<Element> = EQUALIZER_PRESETS
        .iter()
        .map(|(key, name, _)| {
            Button::new(format!("equalizer-preset:{key}"), *name, palette)
                .variant(if chosen == Some(*key) { ButtonVariant::Default } else { ButtonVariant::Secondary })
                .sizing(limn::ButtonSizing { height, ..ControlSize::Regular.button_sizing() })
                .round()
                .render()
        })
        .collect();
    if chosen.is_none() {
        let words = Text::new("Personalizado", palette).font_size(14.0).ink(SCHEME.canvas).render().centered_y();
        let pad = || Element::spacer().size(Some(16.0), None);
        pills.insert(
            0,
            Element::panel(Element::row(vec![pad(), words, pad()]))
                .background(Color::WHITE)
                .radius(height / 2.0)
                .size(None, Some(height))
                .accessible(limn::Access::new(limn::Role::Group).id("equalizer-preset:custom").name("Personalizado")),
        );
    }
    Element::horizontal_scroll_2d("equalizer-presets", Element::row(pills).gap(8.0)).drag_to_pan().scrollbar_visibility(mobile::SCROLL_INDICATOR)
}

/// The preamp's slider, filled from 0 dB, `width` long or as wide as its room.
pub(super) fn preamp(state: &State, width: Option<f32>) -> Element {
    let slider = Slider::new("equalizer:preamp", fraction(state.equalizer.preamp_db), SCHEME.on_raised())
        .origin(0.5)
        .steps(STOPS)
        .ink(Color::WHITE)
        .accessibility_label("Previo");
    match width {
        Some(width) => slider.width(width),
        None => slider,
    }
    .render()
}

/// The window's popover: a heading with a reset, the curves to choose from, the curve over
/// its bands, and the preamp under them.
pub(super) fn popover(state: &State, palette: ComponentPalette) -> Element {
    let heading = Element::row(vec![
        Text::new("Ecualizador", palette).font_size(13.0).render().centered_y(),
        Element::spacer().flex(1.0),
        Button::new("equalizer-reset", "Restablecer", palette).variant(ButtonVariant::Ghost).size(ControlSize::Compact).render(),
    ]);
    let muted = |text: String| Text::new(text, palette).font_size(11.0).muted().render().centered_y();
    let preamp = Element::row(vec![
        muted("Previo".into()).size(Some(40.0), None),
        preamp(state, None).flex(1.0).centered_y(),
        muted(decibels(state.equalizer.preamp_db)).size(Some(40.0), None),
    ])
    .gap(8.0);
    Element::column(vec![heading, presets(state, 28.0), curve(state, 110.0, SCHEME.menu), bands(state, 120.0), preamp])
        .gap(10.0)
        .padding(12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_band_centre_is_a_point_of_the_curve_from_20_hz_to_20_khz() {
        let points = frequencies();
        assert_eq!((points[0], points[points.len() - 1]), (20.0, 20_000.0));
        assert!(points.windows(2).all(|pair| pair[0] < pair[1]));
        for (band, hz) in BAND_HZ.iter().enumerate() {
            assert!((points[band_point(band)] - hz).abs() < 0.01, "{hz} Hz");
        }
        assert_eq!(BAND_HZ.map(band_name), ["31", "62", "125", "250", "500", "1K", "2K", "4K", "8K", "16K"]);
    }
}
