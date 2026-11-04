//! Cropping a picked picture square before it is kept: the whole picture with the square over
//! it, the rest dimmed, moved by a press, a drag or the arrows, its size by a slider. A
//! profile's square shows round, as the face it becomes.

use super::*;
use crate::backend::PictureFor;
use crate::state::Crop;

/// The dimness over what is left out: limn blends in linear light, so it is darker than it reads.
const LEFT_OUT: Color = Color::rgba(0, 0, 0, 185);
/// The square's rim.
const RIM: Color = Color::rgba(255, 255, 255, 235);

/// The picture fitted in `room` a side, with the square over it, and the slider for its size.
pub(crate) fn surface(crop: &Crop, room: f32, palette: ComponentPalette) -> Element {
    let scale = room / crop.width.max(crop.height) as f32;
    let (wide, tall) = (crop.width as f32 * scale, crop.height as f32 * scale);
    let side = crop.side() * scale;
    let (left, top) = (crop.x * wide - side / 2.0, crop.y * tall - side / 2.0);
    let dim = |width: Option<f32>, height: f32| Element::panel(Element::spacer()).background(LEFT_OUT).size(width, Some(height.max(0.0)));
    let left_out = Element::column(vec![
        dim(None, top),
        Element::row(vec![dim(Some(left), side), Element::spacer().size(Some(side), Some(side)), dim(None, side).flex(1.0)]),
        dim(None, tall - top - side).flex(1.0),
    ]);
    let round = matches!(crop.of, PictureFor::Profile);
    let rim = Element::panel(Element::spacer()).border(RIM, 2.0).radius(if round { side / 2.0 } else { 10.0 }).size(Some(side), Some(side));
    let rim = Element::column(vec![Element::spacer().size(None, Some(top)), Element::row(vec![Element::spacer().size(Some(left), None), rim])]);
    let picture = Element::stack(vec![Element::raster_image(crop.image.clone()).size(Some(wide), Some(tall)), left_out, rim]).size(Some(wide), Some(tall));
    // The square's centre goes where the press is, inside the picture: it follows a drag.
    let area = Element::clickable("crop-area", picture).area_slider(crop.x, crop.y, 0.0)
        .accessible(limn::Access::new(limn::Role::Slider).name(t!("common-crop-move")))
        .radius(8.0).clip_children().size(Some(wide), Some(tall));
    let zoom = Slider::new("crop-zoom", crop.zoom, palette).accessibility_label(t!("common-crop-zoom")).render().size(Some(room.min(320.0)), None);
    let glyph = |icon: Icon| Element::icon(icon).foreground(palette.muted).centered_y();
    Element::column(vec![area.centered_x(), Element::row(vec![glyph(Icon::ZoomOut), zoom, glyph(Icon::ZoomIn)]).gap(10.0).centered_x()]).gap(16.0)
}

/// The crop as a dialog: a card on a desktop, a sheet on a phone.
pub(crate) fn dialog(state: &State, window_tree: Element, width: f32) -> Element {
    let Some(crop) = &state.cropping else { return window_tree };
    let palette = dialog_palette();
    if state.mobile {
        let room = (width - 48.0).clamp(200.0, 420.0);
        let footer = dialog_footer(card_button("crop-cancel", t!("common-cancel"), false).render(), card_button("crop-done", t!("common-crop-done"), true).render());
        return limn::BottomSheet::new(window_tree, true, t!("common-crop-title"), t!("common-crop-detail"), "crop-cancel", palette)
            .max_height(820.0).body(surface(crop, room, palette)).footer(footer).render();
    }
    Card { body: Some(surface(crop, 360.0, palette)), go: Some(("crop-done", t!("common-crop-done"), true)), widest: 440.0, focus: Some("crop-area"),
        ..Card::new("crop", t!("common-crop-title"), t!("common-crop-detail"), ("crop-cancel", t!("common-cancel"))) }
        .render(window_tree, width)
}
