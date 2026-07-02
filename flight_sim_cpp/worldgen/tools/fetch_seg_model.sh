#!/usr/bin/env bash
# Fetch the ONNX semantic segmentation model used by the worldgen
# "onnx_semseg" extractor into flight_sim_cpp/data/models/ (gitignored).
#
# Model selection (verified 2026-07-01):
#   The onnx-community SegFormer cityscapes exports referenced in early plans
#   (onnx-community/segformer-b0-finetuned-cityscapes-1024-1024) do NOT exist
#   on huggingface (the resolve URL returns 401 "Invalid username or
#   password", HF's response for a missing repo). The public, genuinely
#   downloadable export used instead:
#
#     repo:  https://huggingface.co/lquint/segformer-b0-finetuned-ade-512-512-onnx
#     file:  onnx/model.onnx (15,259,764 bytes)
#     arch:  SegFormer-B0 fine-tuned on ADE20K at 512x512
#
#   Verified with ONNX Runtime 1.27 (CPU):
#     input  "pixel_values"  float32 NCHW, dynamic H/W (tested 1x3x512x512
#                            and 1x3x128x128); ImageNet normalization
#                            mean (0.485, 0.456, 0.406), std (0.229, 0.224, 0.225)
#     output "logits"        float32 [1, 150, H/4, W/4] (512 input -> 128x128)
#
#   ADE20K classes (0-based) mapped by the extractor's default class_map:
#     1=building, 4=tree, 6=road, 9=grass, 11=sidewalk, 13=earth,
#     21=water, 26=sea, 60=river
#   (Cityscapes lacks water; ADE20K covers all worldgen feature classes.)
#
# Usage: fetch_seg_model.sh [out_file]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_FILE="${1:-${SCRIPT_DIR}/../../data/models/segformer_b0_ade20k_512.onnx}"
URL="https://huggingface.co/lquint/segformer-b0-finetuned-ade-512-512-onnx/resolve/main/onnx/model.onnx"
EXPECTED_MIN_BYTES=15000000

if [ -f "${OUT_FILE}" ]; then
    size="$(wc -c < "${OUT_FILE}" | tr -d ' ')"
    if [ "${size}" -ge "${EXPECTED_MIN_BYTES}" ]; then
        echo "model already present: ${OUT_FILE} (${size} bytes)"
        exit 0
    fi
    echo "existing model looks truncated (${size} bytes); refetching" >&2
fi

mkdir -p "$(dirname "${OUT_FILE}")"
TMP_FILE="${OUT_FILE}.download"
trap 'rm -f "${TMP_FILE}"' EXIT

echo "fetching ${URL}" >&2
curl -sfL "${URL}" -o "${TMP_FILE}"

size="$(wc -c < "${TMP_FILE}" | tr -d ' ')"
if [ "${size}" -lt "${EXPECTED_MIN_BYTES}" ]; then
    echo "download too small (${size} bytes < ${EXPECTED_MIN_BYTES}); aborting" >&2
    exit 1
fi
# ONNX files are protobuf; a quick sanity check on the magic-ish header
# (field 1 = ir_version) guards against HTML error pages.
first_byte="$(head -c 1 "${TMP_FILE}" | od -An -tu1 | tr -d ' ')"
if [ "${first_byte}" != "8" ]; then
    echo "downloaded file does not look like an ONNX protobuf; aborting" >&2
    exit 1
fi

mv "${TMP_FILE}" "${OUT_FILE}"
trap - EXIT
echo "wrote ${OUT_FILE} (${size} bytes)"
