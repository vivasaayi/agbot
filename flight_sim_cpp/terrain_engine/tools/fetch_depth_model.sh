#!/usr/bin/env bash
# Fetch the monocular depth ONNX model used by the mono_depth_onnx elevation
# estimator (terrain_engine). The model is NOT committed to git; the
# flight_sim_cpp/data/ tree is gitignored.
#
# Source: Depth-Anything-V2-Small (ViT-S encoder), ONNX export published at
#   https://huggingface.co/onnx-community/depth-anything-v2-small
#   file: onnx/model.onnx (fp32, ~99 MB, dynamic HxW input "pixel_values",
#          output "predicted_depth" = relative inverse depth)
# License: Depth-Anything-V2-Small is Apache-2.0.
#
# Usage: tools/fetch_depth_model.sh [output_path]
set -euo pipefail

MODEL_URL="https://huggingface.co/onnx-community/depth-anything-v2-small/resolve/main/onnx/model.onnx"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FLIGHT_SIM_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
OUT_PATH="${1:-${FLIGHT_SIM_DIR}/data/models/depth_anything_v2_small.onnx}"
MIN_BYTES=$((20 * 1024 * 1024))

mkdir -p "$(dirname "${OUT_PATH}")"

if [[ -f "${OUT_PATH}" ]]; then
    existing=$(stat -f%z "${OUT_PATH}" 2>/dev/null || stat -c%s "${OUT_PATH}")
    if (( existing > MIN_BYTES )); then
        echo "model already present: ${OUT_PATH} (${existing} bytes)"
        exit 0
    fi
    echo "existing model too small (${existing} bytes); re-downloading"
fi

echo "downloading ${MODEL_URL}"
curl -L --fail --retry 3 -o "${OUT_PATH}.tmp" "${MODEL_URL}"

size=$(stat -f%z "${OUT_PATH}.tmp" 2>/dev/null || stat -c%s "${OUT_PATH}.tmp")
if (( size <= MIN_BYTES )); then
    echo "error: downloaded file too small (${size} bytes); integrity check failed" >&2
    rm -f "${OUT_PATH}.tmp"
    exit 1
fi

mv "${OUT_PATH}.tmp" "${OUT_PATH}"
echo "fetched ${OUT_PATH} (${size} bytes)"
