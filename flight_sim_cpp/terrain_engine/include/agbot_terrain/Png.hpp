#pragma once

#include <cstdint>
#include <filesystem>
#include <string>
#include <vector>

namespace agbot::terrain {

// Minimal, portable PNG decode for cached DEM/map tiles: 8-bit RGB / RGBA /
// greyscale / grey+alpha / palette (PLTE, optional tRNS), non-interlaced.
// Output is always tightly packed
// RGBA. Includes a self-contained DEFLATE (RFC 1951) inflater covering
// stored, fixed-Huffman, and dynamic-Huffman blocks.
struct PngImage {
    bool ok = false;
    std::string error; // reason-coded, e.g. "png_bad_signature", "inflate_bad_huffman"
    int width = 0;
    int height = 0;
    std::vector<std::uint8_t> rgba;
};

struct InflateResult {
    bool ok = false;
    std::string error;
    std::vector<std::uint8_t> bytes;
};

// Raw DEFLATE stream (no zlib wrapper).
[[nodiscard]] InflateResult inflate_deflate_stream(const std::uint8_t* data, std::size_t size);

// zlib stream (RFC 1950): 2-byte header + DEFLATE + Adler-32 trailer.
[[nodiscard]] InflateResult inflate_zlib_stream(const std::uint8_t* data, std::size_t size);

[[nodiscard]] PngImage decode_png_rgba(const std::vector<std::uint8_t>& file_bytes);
[[nodiscard]] PngImage decode_png_rgba_file(const std::filesystem::path& path);

} // namespace agbot::terrain
