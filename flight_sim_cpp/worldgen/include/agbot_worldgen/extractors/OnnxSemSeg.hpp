#pragma once

#include "agbot_worldgen/FeatureExtractor.hpp"

namespace agbot::worldgen {

// Semantic segmentation extractor backed by ONNX Runtime. Compile-gated:
// when the build lacks ONNX Runtime (AGBOT_WORLDGEN_HAS_ONNX undefined) a
// stub registers under the same id and returns the reason code
// "onnx_runtime_unavailable".
//
// Default model (fetched by worldgen/tools/fetch_seg_model.sh):
//   SegFormer-B0 fine-tuned on ADE20K at 512x512
//   https://huggingface.co/lquint/segformer-b0-finetuned-ade-512-512-onnx
//   input  "pixel_values" float32 NCHW, dynamic H/W (ImageNet normalization)
//   output "logits"       float32 [1, 150, H/4, W/4]
//
// Parameters (all read from ExtractionContext::params):
//   model_path         string  ONNX model (default
//                              <src>/data/models/segformer_b0_ade20k_512.onnx)
//   imagery_path       string  required; PNG spanning the AOI (row 0 = north)
//   input_size         int     square model input side (default 512; clamped
//                              to 64..2048 and rounded down to a multiple
//                              of 32)
//   overlap_px         int     tile overlap (default 64)
//   class_map          table   model class index (stringified int key) ->
//                              feature class name. Default ADE20K subset:
//                              1=building, 4=vegetation, 6=road,
//                              9=vegetation, 11=road, 13=bare, 21=water,
//                              26=water, 60=water
//   execution_provider string  "cpu" (default) | "coreml"
//   norm_mean          [f,f,f] channel means (default ImageNet)
//   norm_std           [f,f,f] channel stddevs (default ImageNet)
//   min_area_m2        float   drop smaller blobs (default 10)
//   simplify_tol_m     float   Douglas-Peucker tolerance, 0 = off (default 0)
//   default_confidence float   fallback confidence when a blob carries no
//                              probability mass (default 0.5)
//
// Pipeline: decode PNG -> plan overlapping tiles (SegTiler) -> one ORT run
// per tile (cached session) -> per-tile argmax class + softmax confidence at
// logits resolution, nearest-neighbor upsampled to tile pixels -> stitched
// with center-crop priority (each pixel keeps the tile in which it lies
// deepest) -> per mapped class: polygonize -> features. Feature confidence
// is the mean softmax probability of the assigned class over the blob.
// Tiles smaller than input_size (image smaller than the model input) are
// placed top-left and padded with the normalized-zero color.
class OnnxSemSegExtractor final : public FeatureExtractor {
public:
    static constexpr const char* kId = "onnx_semseg";

    [[nodiscard]] std::string id() const override;
    [[nodiscard]] std::vector<FeatureClass> produces() const override;
    [[nodiscard]] ExtractionResult extract(const ExtractionContext& context) const override;
};

} // namespace agbot::worldgen
