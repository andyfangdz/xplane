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
        (2147483647.9, i32::MAX),
        (-2147483648.9, i32::MIN),
        (2147483648.0, i32::MIN),
        (f64::NAN, i32::MIN),
        (f64::INFINITY, i32::MIN),
        (f64::NEG_INFINITY, i32::MIN),
    ] {
        assert_eq!(fp::truncate_i32(input), output);
    }
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
    assert_eq!(fp::pow(2.0, 3.0), 8.0);
    assert_eq!(fp::powf(4.0, 0.5), 2.0);
    assert_eq!(fp::pow(-0.0, 3.0).to_bits(), (-0.0f64).to_bits());
    assert_eq!(fp::powf(-0.0, 3.0).to_bits(), (-0.0f32).to_bits());
    assert!(fp::pow(-1.0, 0.5).is_nan());
    assert_eq!(fp::tan(-0.0).to_bits(), (-0.0f64).to_bits());
    assert_eq!(fp::fmod(725.5, 360.0), 5.5);
    assert_eq!(fp::fmod(-725.5, 360.0), -5.5);
    assert_eq!(fp::fmod(-0.0, 360.0).to_bits(), (-0.0f64).to_bits());
    assert!(fp::fmod(1.0, 0.0).is_nan());
    assert!(fp::fmod(f64::INFINITY, 1.0).is_nan());
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
    assert_eq!(fp::atanf(-0.0).to_bits(), (-0.0f32).to_bits());
    assert_eq!(fp::atanf(1.0), std::f32::consts::FRAC_PI_4);
    assert!(fp::atanf(f32::NAN).is_nan());
    assert_eq!(fp::atan2f(-0.0, 1.0).to_bits(), (-0.0f32).to_bits());
    assert_eq!(fp::ceil(1.25), 2.0);
    assert_eq!(fp::ceil(-1.25), -1.0);
    assert_eq!(fp::ceil(-0.0).to_bits(), (-0.0f64).to_bits());
    assert_eq!(fp::ceil(f64::INFINITY), f64::INFINITY);
    assert!(fp::ceil(f64::NAN).is_nan());
}

#[test]
fn constant_unary_inputs_use_the_actual_platform_runtime() {
    unsafe extern "C" {
        #[link_name = "cosf"]
        fn native_cos(value: f32) -> f32;
        #[link_name = "sinf"]
        fn native_sin(value: f32) -> f32;
    }
    // These nontrivial binary32 angles expose optimizer folding through a
    // wider-precision implementation instead of the platform binary32 CRT.
    let cosine: unsafe extern "C" fn(f32) -> f32 = std::hint::black_box(native_cos);
    let sine: unsafe extern "C" fn(f32) -> f32 = std::hint::black_box(native_sin);
    // SAFETY: The C math routines accept all scalar floating-point inputs.
    unsafe {
        assert_eq!(
            fp::cosf(f32::from_bits(0xc0c451ec)).to_bits(),
            cosine(f32::from_bits(0xc0c451ec)).to_bits()
        );
        assert_eq!(
            fp::sinf(f32::from_bits(0xc0a8a3d7)).to_bits(),
            sine(f32::from_bits(0xc0a8a3d7)).to_bits()
        );
        assert_eq!(
            fp::sinf(f32::from_bits(0x40c451ec)).to_bits(),
            sine(f32::from_bits(0x40c451ec)).to_bits()
        );
        assert_eq!(
            fp::cosf(f32::from_bits(0xc0a8a3d7)).to_bits(),
            cosine(f32::from_bits(0xc0a8a3d7)).to_bits()
        );
    }
}

#[test]
#[cfg(target_arch = "x86_64")]
#[allow(deprecated)]
fn binary32_angles_match_runtime_in_all_rounding_modes() {
    unsafe extern "C" {
        #[link_name = "atan2f"]
        fn native_angle(y: f32, x: f32) -> f32;
    }
    struct Restore(u32);
    impl Drop for Restore {
        fn drop(&mut self) {
            // SAFETY: Restore the calling thread's saved floating-point state.
            unsafe { std::arch::x86_64::_mm_setcsr(self.0) }
        }
    }
    // SAFETY: MXCSR belongs to this test thread; every exception stays masked.
    let _restore = Restore(unsafe { std::arch::x86_64::_mm_getcsr() });
    let reference: unsafe extern "C" fn(f32, f32) -> f32 = std::hint::black_box(native_angle);
    let values = [
        0.,
        -0.,
        1.,
        -1.,
        0.3,
        -0.7,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::from_bits(0xff800123),
    ];
    for mode in [0x1f80, 0x3f80, 0x5f80, 0x7f80] {
        // SAFETY: These are valid MXCSR settings with all exceptions masked.
        unsafe { std::arch::x86_64::_mm_setcsr(mode) };
        for y in values {
            for x in values {
                // SAFETY: The platform math routine accepts every scalar bit pattern.
                let expected = unsafe { reference(y, x) };
                assert_eq!(fp::atan2f(y, x).to_bits(), expected.to_bits());
                // Status flags may change; control settings must remain intact.
                assert_eq!(unsafe { std::arch::x86_64::_mm_getcsr() } & !0x3f, mode);
            }
        }
    }
}

#[test]
#[cfg(target_arch = "x86_64")]
#[allow(deprecated)]
fn scalar_maximum_matches_sse_values_and_status_with_denormal_modes() {
    use std::arch::x86_64::{_mm_cvtss_f32, _mm_getcsr, _mm_max_ss, _mm_set_ss, _mm_setcsr};
    struct Restore(u32);
    impl Drop for Restore {
        fn drop(&mut self) {
            unsafe { _mm_setcsr(self.0) }
        }
    }
    // SAFETY: All tested environments mask exceptions, and this guard restores
    // the calling thread's original state even if an assertion fails.
    let _restore = Restore(unsafe { _mm_getcsr() });
    let values = [
        0.,
        -0.,
        1.,
        -1.,
        f32::from_bits(1),
        f32::from_bits(0x80000001),
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::from_bits(0x7fc00123),
        f32::from_bits(0x7f800123),
    ];
    for rounding in [0, 0x2000, 0x4000, 0x6000] {
        for denormals in [0, 0x40, 0x8000, 0x8040] {
            let mode = 0x1f80 | rounding | denormals;
            for a in values {
                for b in values {
                    let a = std::hint::black_box(a);
                    let b = std::hint::black_box(b);
                    // SAFETY: SSE is baseline on x86-64; scalar registers only.
                    let (expected, status) = unsafe {
                        _mm_setcsr(mode);
                        let value = _mm_cvtss_f32(_mm_max_ss(_mm_set_ss(a), _mm_set_ss(b)));
                        (value, _mm_getcsr())
                    };
                    unsafe { _mm_setcsr(mode) };
                    let actual = fp::max_second_f32(a, b);
                    let actual_status = unsafe { _mm_getcsr() };
                    assert_eq!(actual.to_bits(), expected.to_bits());
                    assert_eq!(actual_status, status);
                }
            }
        }
    }
}

#[test]
#[cfg(target_arch = "x86_64")]
#[allow(deprecated)]
fn hypotenuse_uses_runtime_scaling_and_preserves_the_caller_environment() {
    unsafe extern "C" {
        #[link_name = "hypot"]
        fn native_hypot(x: f64, y: f64) -> f64;
    }
    struct Restore(u32);
    impl Drop for Restore {
        fn drop(&mut self) {
            unsafe { std::arch::x86_64::_mm_setcsr(self.0) }
        }
    }
    // SAFETY: All tested environments mask exceptions; restore this thread's
    // original state on every exit path.
    let _restore = Restore(unsafe { std::arch::x86_64::_mm_getcsr() });
    let reference: unsafe extern "C" fn(f64, f64) -> f64 = std::hint::black_box(native_hypot);
    let values = [
        0.,
        -0.,
        3.,
        -4.,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ];
    for mode in [0x1f80, 0x3f80, 0x5f80, 0x7f80] {
        for x in values {
            for y in values {
                // SAFETY: The CRT accepts all floating-point inputs. Its internal
                // scaling and status behavior are the contract being compared.
                let (expected, status) = unsafe {
                    std::arch::x86_64::_mm_setcsr(mode);
                    let value = reference(x, y);
                    (value, std::arch::x86_64::_mm_getcsr())
                };
                unsafe { std::arch::x86_64::_mm_setcsr(mode) };
                assert_eq!(fp::hypot(x, y).to_bits(), expected.to_bits());
                assert_eq!(unsafe { std::arch::x86_64::_mm_getcsr() }, status);
            }
        }
    }
    assert!(fp::hypot(f64::MAX, 0.).is_finite());
    assert_eq!(fp::hypot(3., 4.), 5.);
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
        assert_eq!(fp::truncate_i32(-3.9), -3);
        let exact = 9007199254740992.0f64;
        assert_eq!(
            fp::from_i64(9007199254740993).to_bits(),
            exact.to_bits() + u64::from(mode == 0x4000)
        );
        assert_eq!(
            fp::from_i64(-9007199254740993).to_bits(),
            (-exact).to_bits() + u64::from(mode == 0x2000)
        );
        assert_eq!(fp::from_i64(i64::MIN), -9223372036854775808.0);
        assert_eq!(
            fp::from_i64(i64::MAX).to_bits(),
            9223372036854775808.0f64.to_bits() - u64::from(mode == 0x2000 || mode == 0x6000)
        );
        assert_eq!(fp::from_i64(0).to_bits(), 0);
        let controls = Environment::save().0 & !0x3f;
        std::hint::black_box(fp::pow(1.125, 2.0));
        std::hint::black_box(fp::powf(1.125, 3.5));
        std::hint::black_box(fp::tan(0.3));
        std::hint::black_box(fp::atanf(0.3));
        assert_eq!(fp::ceil(1.25), 2.0);
        assert_eq!(fp::ceil(-1.25), -1.0);
        assert_eq!(fp::fmod(725.5, 360.0), 5.5);
        assert_eq!(Environment::save().0 & !0x3f, controls);
        let one32 = 1.0f32.to_bits();
        let half32 = f32::from_bits((127 - 24) << 23);
        let next32 = f32::from_bits(one32 + 1);
        assert_eq!(
            fp::add_f32(1.0, half32).to_bits(),
            one32 + u32::from(mode == 0x4000)
        );
        assert_eq!(
            fp::sub_f32(-1.0, half32).to_bits(),
            (-1.0f32).to_bits() + u32::from(mode == 0x2000)
        );
        assert_eq!(
            fp::mul_f32(next32, next32).to_bits(),
            one32 + 2 + u32::from(mode == 0x4000)
        );
        assert_eq!(
            fp::div_f32(1.0, 10.0).to_bits(),
            0x3dcccccd - u32::from(mode == 0x2000 || mode == 0x6000)
        );
        assert_eq!(
            fp::narrow_f32(0.1).to_bits(),
            0x3dcccccd - u32::from(mode == 0x2000 || mode == 0x6000)
        );
        assert_eq!(fp::mul_f32(0.0, -1.0).to_bits(), (-0.0f32).to_bits());
        assert_eq!(
            fp::add_f32(0.0, -0.0).to_bits(),
            if mode == 0x2000 {
                (-0.0f32).to_bits()
            } else {
                0
            }
        );
        assert_eq!(fp::narrow_f32(-0.0).to_bits(), (-0.0f32).to_bits());
        assert_eq!(
            fp::narrow_f32(f64::MAX).to_bits(),
            if mode == 0x2000 || mode == 0x6000 {
                f32::MAX.to_bits()
            } else {
                f32::INFINITY.to_bits()
            }
        );
        assert_eq!(
            fp::narrow_f32(-f64::MAX).to_bits(),
            if mode == 0x4000 || mode == 0x6000 {
                (-f32::MAX).to_bits()
            } else {
                f32::NEG_INFINITY.to_bits()
            }
        );
    }
    Environment::set(environment.0);
    assert_eq!(Environment::save().0, environment.0);
}
