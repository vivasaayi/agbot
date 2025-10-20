// Minimal unit tests for index edge cases and thermal calibration math

#[test]
fn ndvi_handles_zero_denominator() {
    // (n - r) / (n + r) when n = r = 0 should not panic and yield 0/NODATA per our logic
    let r = 0.0f32;
    let n = 0.0f32;
    let denom = n + r;
    let v = if denom.abs() > f32::EPSILON {
        (n - r) / denom
    } else {
        0.0
    };
    assert_eq!(v, 0.0);
}

#[test]
fn ndvi_basic_values() {
    // Simple sanity: n=1, r=0 -> 1; n=0, r=1 -> -1; n=r -> 0
    assert!(((1.0f32 - 0.0) / (1.0 + 0.0) - 1.0).abs() < 1e-6);
    assert!(((0.0f32 - 1.0) / (0.0 + 1.0) + 1.0).abs() < 1e-6);
    assert!(((0.5f32 - 0.5) / (0.5 + 0.5) - 0.0).abs() < 1e-6);
}

#[test]
fn thermal_bt_from_radiance() {
    // TB = K2 / ln(1 + K1/L)
    let (k1, k2) = (774.8853f32, 1321.0789f32);
    let l = 10.0f32; // radiance
    let tb = k2 / ((k1 / l).ln_1p());
    assert!(tb.is_finite() && tb > 0.0);
}

#[test]
fn emissivity_correction_sane() {
    // LST = TB / (1 + (lambda * TB / rho) * ln(eps))
    let tb = 300.0f64;
    let rho = 1.4388e-2f64;
    let lambda_um = 10.895f64;
    let lambda_m = lambda_um * 1e-6;
    let eps = 0.98f64;
    let lst_k = tb / (1.0 + (lambda_m * tb / rho) * eps.ln());
    // For eps < 1, LST should be slightly higher than TB
    assert!(lst_k > tb - 0.01);
}
