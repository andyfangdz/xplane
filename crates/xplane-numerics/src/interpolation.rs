//! Clamped, piecewise linear interpolation on finite, increasing tables.
#![forbid(unsafe_code)]
use super::fp::{add, div, mul, sub};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidInput,
    InvalidTable,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid interpolation input",
            Self::InvalidTable => "invalid interpolation table",
        })
    }
}
impl std::error::Error for Error {}

/// Interpolate a segment in fraction, difference, product, sum order.
///
/// Uses runtime scalar operations from [`super::fp`]. Does not validate the
/// endpoints, clamp the query or reject NaNs. Equal abscissae divide by zero.
pub fn segment([x0, y0]: [f64; 2], [x1, y1]: [f64; 2], x: f64) -> f64 {
    add(mul(div(sub(x, x0), sub(x1, x0)), sub(y1, y0)), y0)
}

/// Validate the table and query, then clamp to its endpoint ordinates.
///
/// A singleton is a constant function. Coordinates must be finite and
/// abscissae strictly increasing. The last node is evaluated through the final
/// segment when `x` equals its abscissa. Arithmetic can overflow even with finite
/// coordinates; validation does not promise a finite result.
pub fn linear(points: &[[f64; 2]], x: f64) -> Result<f64, Error> {
    if points.is_empty() || !x.is_finite() {
        return Err(Error::InvalidInput);
    }
    for (i, [px, py]) in points.iter().enumerate() {
        if !px.is_finite() || !py.is_finite() || (i != 0 && *px <= points[i - 1][0]) {
            return Err(Error::InvalidTable);
        }
    }
    clamped(points, x).ok_or(Error::InvalidInput)
}

/// The lookup behind [`linear`], without validation.
///
/// A query at or below the first abscissa returns the first ordinate, and one
/// above the last abscissa the last ordinate. Otherwise bisect the table: an
/// abscissa hit at a probe returns its ordinate, and the bracketing pair is
/// evaluated by [`segment`]. The table is assumed increasing, so an unsorted
/// one gives an arbitrary bracket. A NaN `x` fails every comparison, so it
/// returns the ordinate of the first probed point, or NaN from the segment of
/// a two-point table. Returns `None` for an empty table, and for a NaN `x`
/// against a single point.
pub fn clamped(points: &[[f64; 2]], x: f64) -> Option<f64> {
    let (first, last) = (points.first()?, points.last()?);
    if x <= first[0] {
        return Some(first[1]);
    }
    if x > last[0] {
        return Some(last[1]);
    }
    let (mut lo, mut hi) = (0, points.len() - 1);
    loop {
        let mid = lo + (hi - lo) / 2;
        let [px, py] = points[mid];
        if mid == lo || mid == hi {
            return Some(segment(points[mid], *points.get(mid + 1)?, x));
        }
        if x > px {
            lo = mid;
        } else if x < px {
            hi = mid;
        } else {
            return Some(py);
        }
    }
}
