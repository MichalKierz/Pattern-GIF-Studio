pub const TAU: f32 = std::f32::consts::TAU;

#[inline]
pub fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp01((x - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub fn fract(value: f32) -> f32 {
    value - value.floor()
}

#[inline]
pub fn rotate(x: f32, y: f32, angle: f32) -> (f32, f32) {
    let (sin, cos) = angle.sin_cos();
    (x * cos - y * sin, x * sin + y * cos)
}

#[inline]
pub fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

#[inline]
pub fn hash01(seed: u32, x: i32, y: i32) -> f32 {
    let mixed = seed ^ (x as u32).wrapping_mul(0x9e37_79b9) ^ (y as u32).wrapping_mul(0x85eb_ca6b);
    hash_u32(mixed) as f32 / u32::MAX as f32
}
