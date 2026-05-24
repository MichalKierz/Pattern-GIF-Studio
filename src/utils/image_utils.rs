use crate::render::frame_buffer::FrameBuffer;

pub fn frame_to_color_image(frame: &FrameBuffer) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied(
        [frame.width as usize, frame.height as usize],
        &frame.pixels,
    )
}
