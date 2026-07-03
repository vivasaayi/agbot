#include "agbot_terrain/GeoTiff.hpp"

#include <cmath>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <map>
#include <vector>

namespace agbot::terrain {

namespace {

// Endian-aware little/big readers over a byte buffer.
struct Reader {
    const std::vector<unsigned char>& bytes;
    bool little = true;

    [[nodiscard]] bool in_range(std::size_t off, std::size_t len) const {
        return off + len <= bytes.size();
    }
    [[nodiscard]] std::uint16_t u16(std::size_t off) const {
        const std::uint16_t a = bytes[off];
        const std::uint16_t b = bytes[off + 1];
        return little ? static_cast<std::uint16_t>(a | (b << 8))
                      : static_cast<std::uint16_t>((a << 8) | b);
    }
    [[nodiscard]] std::uint32_t u32(std::size_t off) const {
        const std::uint32_t a = bytes[off];
        const std::uint32_t b = bytes[off + 1];
        const std::uint32_t c = bytes[off + 2];
        const std::uint32_t d = bytes[off + 3];
        return little ? (a | (b << 8) | (c << 16) | (d << 24))
                      : (d | (c << 8) | (b << 16) | (a << 24));
    }
    [[nodiscard]] float f32(std::size_t off) const {
        const std::uint32_t bits = u32(off);
        float value = 0.0f;
        std::memcpy(&value, &bits, sizeof(value));
        return value;
    }
    [[nodiscard]] double f64(std::size_t off) const {
        std::uint64_t bits = 0;
        for (int i = 0; i < 8; ++i) {
            const std::uint64_t byte = bytes[off + static_cast<std::size_t>(i)];
            bits |= little ? (byte << (8 * i)) : (byte << (8 * (7 - i)));
        }
        double value = 0.0;
        std::memcpy(&value, &bits, sizeof(value));
        return value;
    }
};

// One decoded IFD entry with its raw (tag, type, count, value/offset) fields.
struct Entry {
    std::uint16_t type = 0;
    std::uint32_t count = 0;
    std::size_t value_field = 0;   // byte offset of the 4-byte value/offset field
};

std::size_t type_size(std::uint16_t type) {
    switch (type) {
        case 1: case 2: case 6: case 7: return 1;   // BYTE/ASCII/SBYTE/UNDEFINED
        case 3: case 8: return 2;                   // SHORT/SSHORT
        case 4: case 9: case 11: return 4;          // LONG/SLONG/FLOAT
        case 5: case 10: case 12: return 8;         // RATIONAL/SRATIONAL/DOUBLE
        default: return 0;
    }
}

// Resolves where an entry's data lives (inline in the value field when it fits
// in 4 bytes, otherwise at the offset stored there).
std::size_t data_offset(const Reader& r, const Entry& e) {
    const std::size_t total = static_cast<std::size_t>(e.count) * type_size(e.type);
    return total <= 4 ? e.value_field : r.u32(e.value_field);
}

std::vector<std::uint64_t> read_uints(const Reader& r, const Entry& e) {
    std::vector<std::uint64_t> out;
    const std::size_t base = data_offset(r, e);
    const std::size_t sz = type_size(e.type);
    if (sz == 0 || !r.in_range(base, static_cast<std::size_t>(e.count) * sz)) {
        return out;
    }
    out.reserve(e.count);
    for (std::uint32_t i = 0; i < e.count; ++i) {
        const std::size_t off = base + static_cast<std::size_t>(i) * sz;
        out.push_back(sz == 2 ? r.u16(off) : r.u32(off));
    }
    return out;
}

std::vector<double> read_doubles(const Reader& r, const Entry& e) {
    std::vector<double> out;
    const std::size_t base = data_offset(r, e);
    if (e.type != 12 || !r.in_range(base, static_cast<std::size_t>(e.count) * 8)) {
        return out;
    }
    out.reserve(e.count);
    for (std::uint32_t i = 0; i < e.count; ++i) {
        out.push_back(r.f64(base + static_cast<std::size_t>(i) * 8));
    }
    return out;
}

std::string read_ascii(const Reader& r, const Entry& e) {
    const std::size_t base = data_offset(r, e);
    if (!r.in_range(base, e.count)) {
        return {};
    }
    std::string out(reinterpret_cast<const char*>(&r.bytes[base]), e.count);
    if (!out.empty() && out.back() == '\0') {
        out.pop_back();
    }
    return out;
}

} // namespace

GeoTiffResult read_geotiff_dem(const std::filesystem::path& path) {
    GeoTiffResult result;
    std::ifstream in(path, std::ios::binary);
    if (!in) {
        result.error = "geotiff_not_found";
        return result;
    }
    std::vector<unsigned char> bytes((std::istreambuf_iterator<char>(in)),
                                     std::istreambuf_iterator<char>());
    if (bytes.size() < 8) {
        result.error = "geotiff_truncated";
        return result;
    }

    Reader r{bytes};
    if (bytes[0] == 'I' && bytes[1] == 'I') {
        r.little = true;
    } else if (bytes[0] == 'M' && bytes[1] == 'M') {
        r.little = false;
    } else {
        result.error = "geotiff_bad_magic";
        return result;
    }
    if (r.u16(2) != 42) {
        result.error = "geotiff_bad_magic";
        return result;
    }

    const std::size_t ifd = r.u32(4);
    if (!r.in_range(ifd, 2)) {
        result.error = "geotiff_bad_ifd";
        return result;
    }
    const std::uint16_t entry_count = r.u16(ifd);
    std::map<std::uint16_t, Entry> tags;
    for (std::uint16_t i = 0; i < entry_count; ++i) {
        const std::size_t off = ifd + 2 + static_cast<std::size_t>(i) * 12;
        if (!r.in_range(off, 12)) {
            break;
        }
        Entry e;
        const std::uint16_t tag = r.u16(off);
        e.type = r.u16(off + 2);
        e.count = r.u32(off + 4);
        e.value_field = off + 8;
        tags[tag] = e;
    }

    const auto scalar = [&](std::uint16_t tag, std::uint64_t fallback) -> std::uint64_t {
        const auto it = tags.find(tag);
        if (it == tags.end()) {
            return fallback;
        }
        const auto values = read_uints(r, it->second);
        return values.empty() ? fallback : values.front();
    };

    if (tags.find(256) == tags.end() || tags.find(257) == tags.end()) {
        result.error = "geotiff_missing_dimensions";
        return result;
    }
    const int width = static_cast<int>(scalar(256, 0));
    const int height = static_cast<int>(scalar(257, 0));
    const std::uint64_t bits = scalar(258, 32);
    const std::uint64_t compression = scalar(259, 1);
    const std::uint64_t samples = scalar(277, 1);
    const std::uint64_t sample_format = scalar(339, 1);
    if (width <= 0 || height <= 0) {
        result.error = "geotiff_bad_dimensions";
        return result;
    }
    if (compression != 1) {
        result.error = "geotiff_unsupported_compression";
        return result;
    }
    if (samples != 1) {
        result.error = "geotiff_unsupported_samples";
        return result;
    }
    if (sample_format != 3 || (bits != 32 && bits != 64)) {
        result.error = "geotiff_not_float";
        return result;
    }
    const std::size_t bytes_per_sample = bits == 64 ? 8 : 4;
    const auto sample_at = [&](std::size_t off) -> float {
        return bits == 64 ? static_cast<float>(r.f64(off)) : r.f32(off);
    };

    // Optional GDAL_NODATA (ASCII) + generic float-nodata sentinel.
    double nodata_value = 0.0;
    bool has_nodata = false;
    if (const auto it = tags.find(42113); it != tags.end() && it->second.type == 2) {
        try {
            nodata_value = std::stod(read_ascii(r, it->second));
            has_nodata = true;
        } catch (...) {
            has_nodata = false;
        }
    }
    const auto is_nodata_sample = [&](float value) {
        if (!std::isfinite(value) || value < -1e30f || value > 1e30f) {
            return true;
        }
        return has_nodata &&
            std::abs(static_cast<double>(value) - nodata_value) <= 1e-6;
    };

    Raster raster = Raster::filled(width, height, GeoBounds{}, Raster::nodata());

    const auto place = [&](std::size_t data_off, int gx, int gy) {
        if (gx < 0 || gy < 0 || gx >= width || gy >= height) {
            return;
        }
        if (!r.in_range(data_off, bytes_per_sample)) {
            return;
        }
        const float value = sample_at(data_off);
        raster.set(gy, gx, is_nodata_sample(value) ? Raster::nodata() : value);
    };

    if (tags.find(322) != tags.end() && tags.find(324) != tags.end()) {
        // Tiled layout.
        const int tile_w = static_cast<int>(scalar(322, 0));
        const int tile_h = static_cast<int>(scalar(323, 0));
        const auto offsets = read_uints(r, tags[324]);
        if (tile_w <= 0 || tile_h <= 0 || offsets.empty()) {
            result.error = "geotiff_bad_tiling";
            return result;
        }
        const int tiles_across = (width + tile_w - 1) / tile_w;
        for (std::size_t t = 0; t < offsets.size(); ++t) {
            const int tx = static_cast<int>(t % static_cast<std::size_t>(tiles_across)) * tile_w;
            const int ty = static_cast<int>(t / static_cast<std::size_t>(tiles_across)) * tile_h;
            for (int rr = 0; rr < tile_h; ++rr) {
                for (int cc = 0; cc < tile_w; ++cc) {
                    const std::size_t data_off = offsets[t] +
                        (static_cast<std::size_t>(rr) * tile_w + cc) * bytes_per_sample;
                    place(data_off, tx + cc, ty + rr);
                }
            }
        }
    } else if (tags.find(273) != tags.end()) {
        // Stripped layout (full-width rows).
        const int rows_per_strip = static_cast<int>(scalar(278, height));
        const auto offsets = read_uints(r, tags[273]);
        if (rows_per_strip <= 0 || offsets.empty()) {
            result.error = "geotiff_bad_strips";
            return result;
        }
        for (std::size_t s = 0; s < offsets.size(); ++s) {
            const int row0 = static_cast<int>(s) * rows_per_strip;
            for (int rr = 0; rr < rows_per_strip && row0 + rr < height; ++rr) {
                for (int cc = 0; cc < width; ++cc) {
                    const std::size_t data_off = offsets[s] +
                        (static_cast<std::size_t>(rr) * width + cc) * bytes_per_sample;
                    place(data_off, cc, row0 + rr);
                }
            }
        }
    } else {
        result.error = "geotiff_no_pixel_data";
        return result;
    }

    // Georeference from ModelPixelScale (33550) + ModelTiepoint (33922).
    const auto scale = read_doubles(r, tags.count(33550) ? tags[33550] : Entry{});
    const auto tie = read_doubles(r, tags.count(33922) ? tags[33922] : Entry{});
    if (scale.size() >= 2 && tie.size() >= 5) {
        const double sx = scale[0];
        const double sy = scale[1];
        const double lon0 = tie[3] - tie[0] * sx;   // geo lon at pixel col 0
        const double lat0 = tie[4] + tie[1] * sy;   // geo lat at pixel row 0 (north)
        raster.bounds.max_latitude = lat0;
        raster.bounds.min_latitude = lat0 - static_cast<double>(height) * sy;
        raster.bounds.min_longitude = lon0;
        raster.bounds.max_longitude = lon0 + static_cast<double>(width) * sx;
        result.has_georef = true;
    }

    result.raster = std::move(raster);
    result.ok = true;
    return result;
}

} // namespace agbot::terrain
