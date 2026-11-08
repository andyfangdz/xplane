use xplane_numerics::{fp, integration, interpolation, rotation, storage::Fields, vector};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-12, "{a} != {b}");
}
fn vector_close(a: [f64; 3], b: [f64; 3]) {
    for i in 0..3 {
        close(a[i], b[i]);
    }
}

#[test]
fn operand_selection_preserves_zero_sign_and_nan_payload() {
    let nan = f64::from_bits(0x7ff8_0000_0000_0123);
    for select in [fp::min_first, fp::max_first] {
        assert_eq!(select(-0.0, 0.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(select(nan, 2.0).to_bits(), nan.to_bits());
        assert_eq!(select(2.0, nan), 2.0);
    }
    for select in [fp::min_second, fp::max_second] {
        assert_eq!(select(-0.0, 0.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(select(nan, 2.0), 2.0);
        assert_eq!(select(2.0, nan).to_bits(), nan.to_bits());
    }
    assert_eq!(fp::clamp_bounds_first(nan, -3.0, 4.0), -3.0);
    assert_eq!(fp::clamp_bounds_first(8.0, -3.0, 4.0), 4.0);
    assert_eq!(fp::clamp_bounds_first(-8.0, -3.0, 4.0), -3.0);
}

#[test]
fn truncation_handles_boundaries_and_invalid_values() {
    for (input, output) in [
        (3.9, 3),
        (-3.9, -3),
        (-0.0, 0),
        (f64::NAN, i64::MIN),
        (f64::INFINITY, i64::MIN),
        (f64::NEG_INFINITY, i64::MIN),
        (9223372036854775808.0, i64::MIN),
        (-9223372036854775808.0, i64::MIN),
        (
            f64::from_bits(9223372036854775808.0f64.to_bits() - 1),
            i64::MAX - 1023,
        ),
    ] {
        assert_eq!(fp::truncate_i64(input), output);
    }
}

#[test]
fn c_runtime_math_and_fraction_split_keep_their_contracts() {
    let mut integral = 99.0;
    assert_eq!(fp::modf(-2.75, &mut integral), -0.75);
    assert_eq!(integral, -2.0);
    assert_eq!(fp::modf(-0.0, &mut integral).to_bits(), (-0.0f64).to_bits());
    assert_eq!(integral.to_bits(), (-0.0f64).to_bits());
    assert_eq!(fp::sqrt(9.0), 3.0);
    assert!(fp::sqrt(-1.0).is_nan());
    close(fp::sin(0.5), 0.479425538604203);
    close(fp::cos(0.5), 0.8775825618903728);
    close(fp::acos(0.5), std::f64::consts::FRAC_PI_3);
    close(fp::atan2(1.0, 0.0), std::f64::consts::FRAC_PI_2);
    assert_eq!(fp::asinf(0.0), 0.0);
    assert_eq!(fp::cosf(0.0), 1.0);
    assert_eq!(fp::sinf(-0.0).to_bits(), (-0.0f32).to_bits());
}

#[test]
fn interpolation_clamps_searches_and_validates() {
    use interpolation::{Error, linear};
    let table = [[-2.0, 4.0], [0.0, 8.0], [1.0, 5.0], [5.0, 13.0]];
    for (x, y) in [
        (-99.0, 4.0),
        (-2.0, 4.0),
        (-1.0, 6.0),
        (0.0, 8.0),
        (0.5, 6.5),
        (1.0, 5.0),
        (3.0, 9.0),
        (5.0, 13.0),
        (99.0, 13.0),
    ] {
        assert_eq!(linear(&table, x), Ok(y));
    }
    for x in [-10.0, 0.0, 10.0] {
        assert_eq!(
            linear(&[[0.0, -0.0]], x).unwrap().to_bits(),
            (-0.0f64).to_bits()
        );
    }
    assert_eq!(linear(&[], 0.0), Err(Error::InvalidInput));
    assert_eq!(linear(&table, f64::NAN), Err(Error::InvalidInput));
    assert_eq!(linear(&table, f64::INFINITY), Err(Error::InvalidInput));
    for bad in [
        [[0.0, 0.0], [0.0, 2.0]],
        [[1.0, 0.0], [0.0, 2.0]],
        [[0.0, f64::NAN], [1.0, 2.0]],
        [[0.0, 0.0], [f64::INFINITY, 2.0]],
    ] {
        assert_eq!(linear(&bad, 0.5), Err(Error::InvalidTable));
    }
}

#[test]
fn trapezoid_integrates_a_linear_derivative_exactly() {
    let mut value = 0.0;
    for i in 0..64 {
        let start = f64::from(i) / 8.0;
        value = integration::trapezoid(value, 2.0 * start, 2.0 * (start + 0.125), 0.125);
    }
    assert_eq!(value, 64.0);
}

#[test]
fn quaternions_follow_documented_zyx_axis_order() {
    let input = [0.25, -1.5, 2.0];
    for roll in [-1.0f64, 0.0, 0.7] {
        for pitch in [-0.8f64, 0.0, 0.4] {
            for yaw in [-2.5f64, 0.0, 2.0] {
                let q = rotation::from_euler_zyx(roll, pitch, yaw);
                let m = rotation::matrix(q);
                let [x, y, z] = input;
                let (sr, cr) = roll.sin_cos();
                let (sp, cp) = pitch.sin_cos();
                let (sy, cy) = yaw.sin_cos();
                let (x, y, z) = (x, cr * y - sr * z, sr * y + cr * z);
                let (x, y, z) = (cp * x + sp * z, y, -sp * x + cp * z);
                let expected = [cy * x - sy * y, sy * x + cy * y, z];
                vector_close(vector::transform(m, input), expected);
                let transpose = std::array::from_fn(|i| std::array::from_fn(|j| m[j][i]));
                vector_close(vector::transform(transpose, expected), input);
            }
        }
    }
}

#[test]
fn integrated_quaternion_tracks_a_constant_body_rate() {
    let mut attitude = [1.0, 0.0, 0.0, 0.0];
    let mut previous = rotation::derivative(attitude, [0.0, 0.0, 0.2]);
    for _ in 0..10000 {
        let current = rotation::derivative(attitude, [0.0, 0.0, 0.2]);
        attitude = rotation::integrate(attitude, previous, current, 0.0001);
        previous = current;
    }
    let direction = vector::transform(rotation::matrix(attitude), [1.0, 0.0, 0.0]);
    assert!((direction[0] - 0.2f64.cos()).abs() < 1e-10);
    assert!((direction[1] - 0.2f64.sin()).abs() < 1e-10);
    close(attitude.into_iter().map(|v| v * v).sum(), 1.0);
    assert!(rotation::normalize([0.0; 4]).iter().all(|v| v.is_nan()));
}

#[test]
fn angular_rate_conversions_are_inverses_away_from_singularities() {
    let body = [0.3, -0.7, 1.4];
    for pitch in [-1.2f64, 0.0, 1.2] {
        for roll in [-2.0f64, 0.0, 2.0] {
            let (sp, cp) = pitch.sin_cos();
            let (sr, cr) = roll.sin_cos();
            let euler = rotation::body_to_euler_rates(body, cp, sp, cr, sr);
            vector_close(rotation::euler_to_body_rates(euler, cp, sp, cr, sr), body);
        }
    }
    assert!(rotation::body_to_euler_rates(body, 0.0, 1.0, 1.0, 0.0)[2].is_infinite());
}

#[test]
fn byte_records_preserve_unaligned_payloads_and_untouched_bytes() {
    let mut fields = Fields::<64>([0xa5; 64]);
    let nan = f64::from_bits(0x7ff8_0000_1234_5678);
    fields.write_f64(1, nan);
    fields.write_f32(9, -0.0);
    fields.write_u32(13, 0x12345678);
    fields.write_u64(17, 0xfedcba9876543210);
    fields.write_vec3(25, [1.0, -0.0, nan]);
    assert_eq!(fields.read_f64(1).to_bits(), nan.to_bits());
    assert_eq!(fields.read_f32(9).to_bits(), (-0.0f32).to_bits());
    assert_eq!(fields.read_u32(13), 0x12345678);
    assert_eq!(&fields.0[17..25], &0xfedcba9876543210u64.to_ne_bytes());
    assert_eq!(
        fields.read_vec3(25).map(f64::to_bits),
        [1.0, -0.0, nan].map(f64::to_bits)
    );
    assert_eq!(fields.0[0], 0xa5);
    assert_eq!(&fields.0[49..], &[0xa5; 15]);
    assert_eq!(std::mem::size_of::<Fields<64>>(), 64);
    assert_eq!(std::mem::align_of::<Fields<64>>(), 1);
}

#[test]
#[should_panic]
fn byte_records_reject_out_of_bounds_access() {
    Fields::<4>::default().read_f64(0);
}

#[cfg(target_arch = "x86_64")]
#[test]
fn runtime_scalar_operations_observe_all_four_rounding_directions() {
    struct Environment(u32);
    impl Environment {
        fn save() -> Self {
            let mut value = 0u32;
            // SAFETY: writable, aligned four-byte local; SSE2 is baseline x86-64.
            unsafe {
                std::arch::asm!("stmxcsr [{p}]", p = in(reg) &mut value, options(nostack, preserves_flags));
            }
            Self(value)
        }
        fn set(value: u32) {
            // SAFETY: valid MXCSR value derived from the saved environment,
            // with masked exceptions; input is a live four-byte local.
            unsafe {
                std::arch::asm!("ldmxcsr [{p}]", p = in(reg) &value, options(nostack, preserves_flags));
            }
        }
    }
    impl Drop for Environment {
        fn drop(&mut self) {
            Self::set(self.0);
        }
    }
    let environment = Environment::save();
    let one = 1.0f64.to_bits();
    let half_ulp = f64::from_bits((1023 - 53) << 52);
    for mode in [0x0000, 0x2000, 0x4000, 0x6000] {
        Environment::set(((environment.0 | 0x1f80) & !0x6000) | mode);
        assert_eq!(
            fp::add(1.0, half_ulp).to_bits(),
            one + u64::from(mode == 0x4000)
        );
        assert_eq!(
            fp::sub(-1.0, half_ulp).to_bits(),
            (-1.0f64).to_bits() + u64::from(mode == 0x2000)
        );
        let next = f64::from_bits(one + 1);
        assert_eq!(
            fp::mul(next, next).to_bits(),
            one + 2 + u64::from(mode == 0x4000)
        );
        assert_eq!(
            fp::div(1.0, 10.0).to_bits(),
            0x3fb999999999999a - u64::from(mode == 0x2000 || mode == 0x6000)
        );
        assert_eq!(
            interpolation::segment([0.0, 1.0], [2.0, next], 1.0).to_bits(),
            one + u64::from(mode == 0x4000)
        );
        assert_eq!(fp::truncate_i64(-3.9), -3);
    }
    Environment::set(environment.0);
    assert_eq!(Environment::save().0, environment.0);
}
