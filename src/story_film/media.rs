//! The shipped AVI files have one Motion JPEG picture stream (00) and an
//! interleaved German soundtrack for standalone players. In-game audio comes
//! from the selected language's Ogg Vorbis file.
use bevy::{asset::RenderAssetUsages, prelude::Image};
use std::{ops::Range, path::Path, sync::Arc};

pub(super) struct Movie {
    bytes: Vec<u8>,
    frames: Vec<Range<usize>>,
    pub duration: f32,
}

impl Movie {
    pub fn load(path: &Path) -> Result<(Arc<Self>, Image), String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let movie = Arc::new(Self::parse(bytes).ok_or("invalid Motion JPEG AVI")?);
        let image = movie.decode(0)?;
        Ok((movie, image))
    }

    fn parse(bytes: Vec<u8>) -> Option<Self> {
        if bytes.get(..4)? != b"RIFF" || bytes.get(8..12)? != b"AVI " {
            return None;
        }
        let end = usize::try_from(word(&bytes, 4)?).ok()?.checked_add(8)?;
        let mut frames = Vec::new();
        let mut micros = 0;
        chunks(&bytes, 12..end, &mut |id, body| {
            if id != b"LIST" {
                return Some(());
            }
            let kind = bytes.get(body.start..body.start + 4)?;
            let contents = body.start + 4..body.end;
            match kind {
                b"hdrl" => chunks(&bytes, contents, &mut |kind, header| {
                    if kind == b"avih" {
                        micros = word(bytes.get(header)?, 0)?;
                    }
                    Some(())
                }),
                b"movi" => chunks(&bytes, contents, &mut |id, chunk| {
                    if id == b"LIST" && bytes.get(chunk.start..chunk.start + 4)? == b"rec " {
                        chunks(&bytes, chunk.start + 4..chunk.end, &mut |id, frame| {
                            video_chunk(id, frame, &mut frames)
                        })
                    } else {
                        video_chunk(id, chunk, &mut frames)
                    }
                }),
                _ => Some(()),
            }
        })?;
        if frames.is_empty() || micros == 0 {
            return None;
        }
        let duration = micros as f32 * frames.len() as f32 / 1_000_000.;
        Some(Self {
            bytes,
            frames,
            duration,
        })
    }

    pub fn frame_at(&self, fraction: f32) -> usize {
        ((fraction.clamp(0., 1.) * self.frames.len() as f32) as usize).min(self.frames.len() - 1)
    }

    pub fn decode(&self, index: usize) -> Result<Image, String> {
        let image = image::load_from_memory_with_format(
            &self.bytes[self.frames[index].clone()],
            image::ImageFormat::Jpeg,
        )
        .map_err(|e| e.to_string())?;
        Ok(Image::from_dynamic(
            image,
            true,
            RenderAssetUsages::default(),
        ))
    }
}

fn video_chunk(id: &[u8], data: Range<usize>, frames: &mut Vec<Range<usize>>) -> Option<()> {
    if id == b"00dc" || id == b"00db" {
        // AVI's zero-byte frame repeats the previous picture.
        let frame = if data.is_empty() {
            frames.last()?.clone()
        } else {
            data
        };
        frames.push(frame);
    }
    Some(())
}

fn word(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn chunks(
    bytes: &[u8],
    range: Range<usize>,
    visit: &mut impl FnMut(&[u8], Range<usize>) -> Option<()>,
) -> Option<()> {
    let mut at = range.start;
    if range.end > bytes.len() || at > range.end {
        return None;
    }
    while at < range.end {
        let id = bytes.get(at..at + 4)?;
        let length = word(bytes, at + 4)? as usize;
        let payload = at + 8;
        let end = payload.checked_add(length)?;
        if end > range.end {
            return None;
        }
        visit(id, payload..end)?;
        at = end + (length & 1);
    }
    (at == range.end).then_some(())
}

/// Vorbis stores the sample rate in its identification packet. Each Ogg page
/// records how many PCM samples have been played through the end of that page.
pub(super) fn narration_seconds(bytes: &[u8]) -> Option<f32> {
    let mut at = 0;
    let mut rate = None;
    while at < bytes.len() {
        let header = bytes.get(at..at + 27)?;
        if &header[..4] != b"OggS" {
            return None;
        }
        let laces = bytes.get(at + 27..at + 27 + usize::from(header[26]))?;
        let payload = at + 27 + laces.len();
        let end = payload.checked_add(laces.iter().map(|b| usize::from(*b)).sum())?;
        let packet = bytes.get(payload..end)?;
        if rate.is_none() && packet.starts_with(b"\x01vorbis") {
            rate = Some(word(packet, 12)?);
        }
        if header[5] & 4 != 0 {
            let samples = i64::from_le_bytes(header[6..14].try_into().ok()?);
            let hz = rate?;
            return (samples > 0 && hz > 0).then(|| samples as f32 / hz as f32);
        }
        at = end;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avi_skips_audio_and_holds_the_last_picture() {
        fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut bytes = Vec::from(id);
            bytes.extend((payload.len() as u32).to_le_bytes());
            bytes.extend(payload);
            if !payload.len().is_multiple_of(2) {
                bytes.push(0);
            }
            bytes
        }
        let header = chunk(
            b"LIST",
            &[
                b"hdrl".as_slice(),
                &chunk(b"avih", &100_000u32.to_le_bytes()),
            ]
            .concat(),
        );
        let frames = [
            chunk(b"00dc", &[7]),
            chunk(b"01wb", &[1, 2]),
            chunk(b"00dc", &[]),
            chunk(b"00dc", &[9]),
        ]
        .concat();
        let stream = chunk(b"LIST", &[b"movi".as_slice(), &frames].concat());
        let avi = chunk(b"RIFF", &[b"AVI ".as_slice(), &header, &stream].concat());
        let clip = Movie::parse(avi.clone()).unwrap();
        assert_eq!(clip.frames[0], clip.frames[1]);
        assert_eq!(clip.duration, 0.3);
        assert_eq!(clip.frame_at(1.), 2);
        assert_eq!(&clip.bytes[clip.frames[2].clone()], &[9]);
        assert!(Movie::parse(avi[..avi.len() - 1].to_vec()).is_none());
    }

    #[test]
    fn every_shipped_story_film_decodes_at_both_ends() {
        for story in crate::data::GameData::load().stories {
            let path =
                crate::data::assets_dir().join(format!("videos/endings/{}.avi", story.ghost));
            let (movie, first) =
                Movie::load(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let last = movie.decode(movie.frame_at(1.)).unwrap();
            assert_eq!(first.size(), last.size());
            assert!(movie.frames.len() > 1 && movie.duration > 0.);
        }
    }

    #[test]
    fn reads_duration_without_a_sound_device() {
        let bytes =
            std::fs::read(crate::data::assets_dir().join("sounds/intro_voice_de.ogg")).unwrap();
        assert!(narration_seconds(&bytes).is_some_and(|d| (10. ..20.).contains(&d)));
    }
}
