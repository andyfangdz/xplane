//! Three-dimensional vectors and row-major matrices; no simulator axis policy.
#![forbid(unsafe_code)]

pub type Vec3 = [f64; 3];
pub type Matrix3 = [[f64; 3]; 3];

/// Component addition, retaining left/right operand order.
pub fn add(left: Vec3, right: Vec3) -> Vec3 {
    std::array::from_fn(|i| left[i] + right[i])
}

/// Left-associated dot product: `(a0*b0 + a1*b1) + a2*b2`.
pub fn dot(a: Vec3, b: Vec3) -> f64 {
    (a[0] * b[0] + a[1] * b[1]) + a[2] * b[2]
}

pub fn transform(matrix: Matrix3, vector: Vec3) -> Vec3 {
    matrix.map(|row| dot(row, vector))
}
