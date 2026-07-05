#pragma once

#include <vector>

namespace agbot::terrain {

// Result of anchoring a relative (affine-invariant) depth/height signal to a
// metric reference: y ~= a * x + b fitted by least squares with iterative
// sigma clipping (RANSAC-lite). Used by the mono_depth_onnx estimator to
// anchor relative depth to the DEM prior; exposed here so the fit is unit
// testable without ONNX Runtime.
struct AffineFit {
    bool ok = false;
    double a = 0.0;
    double b = 0.0;
    double sigma = 0.0;      // std-dev of inlier residuals (y - (a*x + b))
    int inliers = 0;         // samples surviving the final clipping round
};

// Least-squares fit of y = a*x + b over paired samples, then `rounds`
// re-fits dropping samples whose residual exceeds `sigma_clip` standard
// deviations. Deterministic. Requires >= 2 samples with distinct x values.
[[nodiscard]] AffineFit fit_affine_sigma_clipped(
    const std::vector<float>& x,
    const std::vector<float>& y,
    int rounds = 3,
    double sigma_clip = 2.0);

} // namespace agbot::terrain
