use crate::{export::export_settings::ExportSettings, render::frame_buffer::FrameBuffer};

pub fn quantization_speed(settings: &ExportSettings) -> i32 {
    let quality = settings.lossy_quality.clamp(1, 100);
    let quality_speed = 31 - ((quality as i32 * 30 + 99) / 100);
    if settings.fast {
        (quality_speed + 8).clamp(1, 30)
    } else {
        quality_speed.clamp(1, 30)
    }
}

pub fn apply_gif_quality_preview(mut frame: FrameBuffer, settings: &ExportSettings) -> FrameBuffer {
    let levels = preview_color_levels(settings);
    if levels >= 256 {
        return frame;
    }

    for pixel in frame.pixels.chunks_exact_mut(4) {
        pixel[0] = quantize_channel(pixel[0], levels);
        pixel[1] = quantize_channel(pixel[1], levels);
        pixel[2] = quantize_channel(pixel[2], levels);
    }
    frame
}

fn preview_color_levels(settings: &ExportSettings) -> u16 {
    let mut quality = settings.lossy_quality.clamp(1, 100) as u16;
    if settings.fast {
        quality = quality.saturating_sub(20).max(1);
    }

    if quality >= 100 {
        256
    } else {
        let curved = quality as u32 * quality as u32;
        (8 + ((curved * 248) / 10_000)) as u16
    }
}

fn quantize_channel(value: u8, levels: u16) -> u8 {
    let levels = levels.clamp(2, 256);
    let max_index = levels - 1;
    let index = ((value as u32 * max_index as u32 + 127) / 255) as u16;
    ((index as u32 * 255 + (max_index / 2) as u32) / max_index as u32) as u8
}

#[cfg(test)]
mod tests {
    use crate::{
        export::{
            export_settings::ExportSettings,
            gif_quality::{apply_gif_quality_preview, quantization_speed},
        },
        render::frame_buffer::FrameBuffer,
    };

    #[test]
    fn fast_encode_uses_faster_quantization_but_still_respects_quality() {
        let mut settings = ExportSettings {
            lossy_quality: 100,
            fast: false,
            ..ExportSettings::default()
        };
        assert_eq!(quantization_speed(&settings), 1);

        settings.fast = true;
        assert_eq!(quantization_speed(&settings), 9);

        settings.lossy_quality = 1;
        assert_eq!(quantization_speed(&settings), 30);
    }

    #[test]
    fn gif_quality_preview_preserves_frame_dimensions() {
        let frame =
            FrameBuffer::from_pixels(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 255]).expect("frame");

        let preview = apply_gif_quality_preview(frame, &ExportSettings::default());

        assert_eq!(preview.width, 2);
        assert_eq!(preview.height, 1);
        assert_eq!(preview.pixels.len(), 8);
    }

    #[test]
    fn gif_quality_preview_leaves_max_quality_non_fast_frame_unchanged() {
        let frame = FrameBuffer::from_pixels(1, 1, vec![123, 45, 231, 255]).expect("frame");
        let settings = ExportSettings {
            lossy_quality: 100,
            fast: false,
            ..ExportSettings::default()
        };

        let preview = apply_gif_quality_preview(frame.clone(), &settings);

        assert_eq!(preview, frame);
    }

    #[test]
    fn gif_quality_preview_changes_low_quality_frame_without_expensive_encoding() {
        let frame = FrameBuffer::from_pixels(1, 1, vec![123, 45, 231, 255]).expect("frame");
        let settings = ExportSettings {
            lossy_quality: 1,
            fast: true,
            ..ExportSettings::default()
        };

        let preview = apply_gif_quality_preview(frame.clone(), &settings);

        assert_ne!(preview.pixels, frame.pixels);
        assert_eq!(preview.pixels[3], 255);
    }

    #[test]
    fn gif_quality_preview_does_not_overflow_when_fast_encode_reduces_high_quality() {
        let frame = FrameBuffer::from_pixels(1, 1, vec![123, 45, 231, 255]).expect("frame");
        let settings = ExportSettings {
            lossy_quality: 100,
            fast: true,
            ..ExportSettings::default()
        };

        let preview = apply_gif_quality_preview(frame, &settings);

        assert_eq!(preview.pixels.len(), 4);
    }
}
