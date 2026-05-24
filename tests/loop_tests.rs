use pattern_gif_studio::animation::loop_time::{LoopTime, loop_phase, total_frames};

#[test]
fn loop_phase_never_emits_duplicate_end_frame() {
    let total = 120;
    assert_eq!(loop_phase(0, total), 0.0);
    assert!(loop_phase(total - 1, total) < 1.0);
    assert_eq!(loop_phase(total, total), 0.0);
}

#[test]
fn total_frames_uses_fps_and_duration() {
    assert_eq!(total_frames(24, 4.0), 96);
    assert_eq!(total_frames(30, 5.0), 150);
}

#[test]
fn loop_time_from_frame_is_periodic() {
    let first = LoopTime::from_frame(0, 60);
    let wrapped = LoopTime::from_frame(60, 60);

    assert!((first.phase - wrapped.phase).abs() < f32::EPSILON);
    assert!((first.angle - wrapped.angle).abs() < f32::EPSILON);
}
