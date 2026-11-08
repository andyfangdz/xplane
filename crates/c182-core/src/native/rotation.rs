//! Hamilton quaternions `[w, x, y, z]`, right-handed rotations and ZYX Euler angles.
#![forbid(unsafe_code)]
use super::{fp, integration, vector::Matrix3};

pub type Quaternion = [f64; 4];

/// Construct `Rz(yaw) * Ry(pitch) * Rx(roll)`; angles are radians.
pub fn from_euler_zyx(roll: f64, pitch: f64, yaw: f64) -> Quaternion {
    let (cp, sp, cr, sr, cy, sy) = (
        fp::cos(pitch * 0.5),
        fp::sin(pitch * 0.5),
        fp::cos(roll * 0.5),
        fp::sin(roll * 0.5),
        fp::cos(yaw * 0.5),
        fp::sin(yaw * 0.5),
    );
    let (a, b, c, d) = (cy * cp, cy * sp, sy * cp, sy * sp);
    [
        a * cr + d * sr,
        a * sr - d * cr,
        b * cr + c * sr,
        c * cr - b * sr,
    ]
}

/// Active, row-major rotation matrix. The caller supplies a unit quaternion.
/// Does not normalize implicitly; a non-unit input scales the matrix.
pub fn matrix([w, x, y, z]: Quaternion) -> Matrix3 {
    let (ww, xx, yy, zz) = (w * w, x * x, y * y, z * z);
    let (yw, yx, zw, zy, zx, wx) = (y * w, y * x, z * w, z * y, z * x, w * x);
    let twice = |v| v + v;
    [
        [((ww + xx) - yy) - zz, twice(yx - zw), twice(yw + zx)],
        [twice(zw + yx), ((ww - xx) + yy) - zz, twice(zy - wx)],
        [twice(zx - yw), twice(wx + zy), ((ww - xx) - yy) + zz],
    ]
}

/// Quaternion derivative from body angular velocity `[p, q, r]` in radians/s.
pub fn derivative([w, x, y, z]: Quaternion, [p, q, r]: [f64; 3]) -> Quaternion {
    [
        ((x * p + y * q) + z * r) * -0.5,
        ((w * p + y * r) - z * q) * 0.5,
        ((z * p + w * q) - x * r) * 0.5,
        ((w * r + x * q) - y * p) * 0.5,
    ]
}

/// Normalize with a left-associated squared norm. Zero and nonfinite norms
/// propagate IEEE-754 results; choosing a fallback attitude is caller policy.
pub fn normalize(mut value: Quaternion) -> Quaternion {
    let [w, x, y, z] = value;
    let scale = 1.0 / fp::sqrt(((w * w + x * x) + y * y) + z * z);
    for component in &mut value {
        *component *= scale;
    }
    value
}

/// Integrate using explicit endpoint derivatives, then normalize.
pub fn integrate(value: Quaternion, first: Quaternion, second: Quaternion, dt: f64) -> Quaternion {
    normalize(std::array::from_fn(|i| {
        integration::trapezoid(value[i], first[i], second[i], dt)
    }))
}

/// ZYX Euler angle derivatives `[roll, pitch, yaw]` from body rates `[p,q,r]`.
/// Trigonometric inputs let the caller cache angles and select a denominator
/// near the pitch singularity. `cos_pitch` is used as provided, without clamps.
pub fn body_to_euler_rates(
    [p, q, r]: [f64; 3],
    cos_pitch: f64,
    sin_pitch: f64,
    cos_roll: f64,
    sin_roll: f64,
) -> [f64; 3] {
    let yaw = (sin_roll * q + cos_roll * r) / cos_pitch;
    [yaw * sin_pitch + p, q * cos_roll - sin_roll * r, yaw]
}

/// Body rates `[p,q,r]` from ZYX Euler angle derivatives `[roll,pitch,yaw]`.
pub fn euler_to_body_rates(
    [roll, pitch, yaw]: [f64; 3],
    cos_pitch: f64,
    sin_pitch: f64,
    cos_roll: f64,
    sin_roll: f64,
) -> [f64; 3] {
    [
        roll - sin_pitch * yaw,
        (sin_roll * yaw) * cos_pitch + pitch * cos_roll,
        (cos_pitch * yaw) * cos_roll - sin_roll * pitch,
    ]
}
