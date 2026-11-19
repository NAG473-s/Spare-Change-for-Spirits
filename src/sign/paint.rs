//! Localised signwriting. The UI text layout shapes the letters (including
//! joined scripts and font fallbacks). Only their pigment is transferred to
//! the board, so the wood, nails, lighting and turn of the sign show through.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageLoaderSettings;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::text::TextLayoutInfo;

use crate::data::asset_exists;
use crate::i18n::Lang;
use crate::ui::{UiFont, text_font};

const ART: &str = "sprites/shop/sign_paint.png";
const WIDTH: usize = 512;
const HEIGHT: usize = 144;

/// The rows in the brushwork sheet, in order.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Motif {
    Anchor,
    Awning,
    Lantern,
    Shop,
    Moon,
    Sunrise,
    ShopCrest,
}

#[derive(Component, Clone, Copy)]
struct Inscription {
    key: &'static str,
    motif: Motif,
}

impl Inscription {
    fn words(self, lang: &Lang) -> String {
        let text = lang.t(self.key);
        if !matches!(self.motif, Motif::Lantern | Motif::Shop) {
            return text;
        }
        // The post itself points the way. Share the destination across the two
        // planks at a word boundary; an unbroken name fits on the upper plank.
        let words = text.trim().trim_end_matches('→').trim();
        let (upper, lower) = plank_lines(words);
        match self.motif {
            Motif::Lantern => upper.to_owned(),
            _ => lower.to_owned(),
        }
    }
}

fn plank_lines(words: &str) -> (&str, &str) {
    let length = words.chars().count();
    let split = words
        .char_indices()
        .enumerate()
        .filter(|(_, (_, c))| c.is_whitespace())
        .min_by_key(|(n, _)| (2 * n).abs_diff(length));
    match split {
        Some((_, (index, _))) => (words[..index].trim_end(), words[index..].trim_start()),
        None => (words, ""),
    }
}

pub(crate) fn on_board(
    commands: &mut Commands,
    parent: Entity,
    key: &'static str,
    motif: Motif,
    transform: Transform,
    size: Vec2,
) -> Entity {
    commands
        .spawn((
            Inscription { key, motif },
            transform.with_scale(size.extend(1.0)),
            Visibility::default(),
            NotShadowCaster,
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
}

#[derive(Resource)]
struct SignArt {
    quad: Handle<Mesh>,
    stencil: Option<Handle<Image>>,
}

impl FromWorld for SignArt {
    fn from_world(world: &mut World) -> Self {
        let stencil = asset_exists(ART).then(|| {
            world
                .resource::<AssetServer>()
                .load_builder()
                .with_settings(|s: &mut ImageLoaderSettings| {
                    s.asset_usage = RenderAssetUsages::MAIN_WORLD;
                })
                .load(ART)
        });
        Self {
            quad: world
                .resource_mut::<Assets<Mesh>>()
                .add(Rectangle::default()),
            stencil,
        }
    }
}

#[derive(Component)]
struct Lettering {
    board: Entity,
    inscription: Inscription,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<SignArt>().add_systems(
        Update,
        (prepare, translate.run_if(resource_changed::<UiFont>), paint),
    );
}

fn prepare(
    mut commands: Commands,
    art: Res<SignArt>,
    boards: Query<(Entity, &Inscription), Added<Inscription>>,
    lang: Res<Lang>,
    font: Res<UiFont>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (board, &inscription) in &boards {
        let target = images.add(Image::transparent());
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(target.clone()),
            // A little reflected light keeps the pale paint legible on the night quay.
            emissive_texture: Some(target.clone()),
            emissive: LinearRgba::rgb(0.36, 0.32, 0.27),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.96,
            ..default()
        });
        commands
            .entity(board)
            .insert((Mesh3d(art.quad.clone()), MeshMaterial3d(material)));
        commands.spawn((
            Lettering { board, inscription },
            Text::new(inscription.words(&lang)),
            text_font(&font.display, 54.0),
            TextLayout::no_wrap(),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
        ));
    }
}

fn translate(
    lang: Res<Lang>,
    font: Res<UiFont>,
    mut labels: Query<(&Lettering, &mut Text, &mut TextFont)>,
) {
    for (legend, mut text, mut face) in &mut labels {
        text.0 = legend.inscription.words(&lang);
        face.font = font.display.clone();
    }
}

fn paint(
    art: Res<SignArt>,
    marks: Query<(&Lettering, &Text, Ref<TextLayoutInfo>)>,
    boards: Query<(&GlobalTransform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut had_stencil: Local<bool>,
) {
    let ready = art
        .stencil
        .as_ref()
        .is_some_and(|h| images.contains(h.id()));
    let new_stencil = ready && !*had_stencil;
    *had_stencil = ready;
    for (mark, words, layout) in &marks {
        if !layout.is_changed() && !new_stencil {
            continue;
        }
        let Ok((at, material)) = boards.get(mark.board) else {
            continue;
        };
        let matrix = at.affine().matrix3;
        let aspect = matrix.x_axis.length() / matrix.y_axis.length();
        let mut rgba = vec![0; WIDTH * HEIGHT * 4];
        if let Some(sheet) = art.stencil.as_ref().and_then(|h| images.get(h))
            && sheet.width() == WIDTH as u32
            && sheet.height() == (HEIGHT * (Motif::ShopCrest as usize + 1)) as u32
            && let Some(data) = &sheet.data
        {
            let len = rgba.len();
            let motif = if mark.inscription.motif == Motif::Shop && words.is_empty() {
                Motif::ShopCrest
            } else {
                mark.inscription.motif
            };
            let from = motif as usize * len;
            if let Some(row) = data.get(from..from + len) {
                rgba.copy_from_slice(row);
            }
        }
        if let Some(mask) = InkMask::from_layout(&layout, &images) {
            mask.print(&mut rgba, aspect);
        }
        // Swap the handle on the material as well. Its GPU bind group may still
        // reference the transparent startup texture or the previous language.
        if let Some(mut material) = materials.get_mut(material) {
            let texture = images.add(paint_texture(rgba));
            material.base_color_texture = Some(texture.clone());
            material.emissive_texture = Some(texture);
        }
    }
}

fn paint_texture(rgba: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: WIDTH as u32,
            height: HEIGHT as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    // At the diorama camera each screen pixel covers several brush strokes.
    // Average the covered pigment separately from the transparent wood.
    let data = image
        .data
        .as_mut()
        .expect("the sign texture contains pixels");
    let (mut w, mut h, mut offset) = (WIDTH, HEIGHT, 0);
    while w.is_multiple_of(2) && h.is_multiple_of(2) {
        let next = data.len();
        for y in (0..h).step_by(2) {
            for x in (0..w).step_by(2) {
                let i = y * w + x;
                let mut rgb = [0u32; 3];
                let mut alpha = 0u32;
                for pixel in [i, i + 1, i + w, i + w + 1] {
                    let start = offset + 4 * pixel;
                    let a = u32::from(data[start + 3]);
                    alpha += a;
                    for k in 0..3 {
                        rgb[k] += u32::from(data[start + k]) * a;
                    }
                }
                data.extend(rgb.map(|v| (v / alpha.max(1)) as u8));
                data.push(((alpha + 2) / 4) as u8);
            }
        }
        image.texture_descriptor.mip_level_count += 1;
        (w, h, offset) = (w / 2, h / 2, next);
    }
    image
}

struct InkMask {
    width: usize,
    height: usize,
    alpha: Vec<u8>,
}

impl InkMask {
    fn from_layout(layout: &TextLayoutInfo, images: &Assets<Image>) -> Option<Self> {
        let mut bounds = Rect::EMPTY;
        for glyph in &layout.glyphs {
            bounds = bounds.union(Rect::from_center_size(
                glyph.position,
                glyph.atlas_info.rect.size(),
            ));
        }
        if bounds.is_empty() {
            return None;
        }
        let origin = bounds.min.floor();
        let dims = (bounds.max.ceil() - origin).as_uvec2();
        let (w, h) = (dims.x as usize, dims.y as usize);
        let mut alpha = vec![0; w * h];
        for glyph in &layout.glyphs {
            let atlas = images.get(glyph.atlas_info.texture)?;
            let data = atlas.data.as_ref()?;
            let rect = glyph.atlas_info.rect;
            let dst = (glyph.position - rect.half_size() - origin)
                .round()
                .as_uvec2();
            for y in 0..rect.height() as usize {
                let row = (rect.min.y as usize + y) * atlas.width() as usize + rect.min.x as usize;
                for x in 0..rect.width() as usize {
                    let pixel = &mut alpha[(dst.y as usize + y) * w + dst.x as usize + x];
                    *pixel = (*pixel).max(data[(row + x) * 4 + 3]);
                }
            }
        }
        Some(Self {
            width: w,
            height: h,
            alpha,
        })
    }

    fn sample(&self, x: f32, y: f32) -> f32 {
        let at = |u: i32, v: i32| {
            if u < 0 || v < 0 || u >= self.width as i32 || v >= self.height as i32 {
                return 0.0;
            }
            self.alpha[v as usize * self.width + u as usize] as f32 / 255.0
        };
        let (u, v) = (x.floor() as i32, y.floor() as i32);
        let (s, t) = (x - u as f32, y - v as f32);
        at(u, v)
            .lerp(at(u + 1, v), s)
            .lerp(at(u, v + 1).lerp(at(u + 1, v + 1), s), t)
    }

    fn print(&self, rgba: &mut [u8], aspect: f32) {
        // Leave the emblem and pinstriping uncovered. Preserve the glyph aspect
        // even when the wood has been stretched to fit a wall or a post.
        let pixel_aspect = aspect * HEIGHT as f32 / WIDTH as f32;
        let scale = (HEIGHT as f32 * 0.66 / self.height as f32)
            .min(WIDTH as f32 * 0.73 * pixel_aspect / self.width as f32);
        let sx = scale / pixel_aspect;
        let left = WIDTH as f32 * 0.593 - self.width as f32 * sx * 0.5;
        let top = (HEIGHT as f32 - self.height as f32 * scale) * 0.5;
        let mut fill = vec![0.0; WIDTH * HEIGHT];
        for (i, alpha) in fill.iter_mut().enumerate() {
            *alpha = self.sample(
                ((i % WIDTH) as f32 - left) / sx,
                ((i / WIDTH) as f32 - top) / scale,
            );
        }
        // Dark cut edge, followed by the ochre undercoat and the cream face.
        // Keep the wood visible around and inside every individual letter.
        for y in 3..HEIGHT - 3 {
            for x in 3..WIDTH - 3 {
                let i = y * WIDTH + x;
                let face = fill[i];
                let rim = face
                    .max(fill[i - 2])
                    .max(fill[i + 2])
                    .max(fill[i - 3 * WIDTH])
                    .max(fill[i + 2 * WIDTH]);
                if rim == 0.0 {
                    continue;
                }
                let bevel = (fill[i - WIDTH]).min(face);
                let light = face / rim;
                let rgb = Vec3::new(56., 34., 34.)
                    .lerp(Vec3::new(229., 176., 102.), light)
                    .lerp(Vec3::new(255., 237., 191.), bevel / rim);
                rgba[i * 4..i * 4 + 4].copy_from_slice(&[
                    rgb.x as u8,
                    rgb.y as u8,
                    rgb.z as u8,
                    (rim * 255.) as u8,
                ]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_are_split_only_between_whole_words() {
        assert_eq!(plank_lines("Zurück zum Laden"), ("Zurück", "zum Laden"));
        assert_eq!(plank_lines("عُد إلى المتجر"), ("عُد إلى", "المتجر"));
        assert_eq!(plank_lines("店に戻る"), ("店に戻る", ""));
    }
}
