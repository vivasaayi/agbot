#include "agbot_terrain/Png.hpp"

#include <algorithm>
#include <array>
#include <cstring>
#include <fstream>

namespace agbot::terrain {
namespace {

// ---------------------------------------------------------------------------
// DEFLATE (RFC 1951) — compact puff-style inflater.
// ---------------------------------------------------------------------------

constexpr int kMaxBits = 15;
constexpr int kMaxLitLenCodes = 288;
constexpr int kMaxDistCodes = 30;
constexpr int kMaxCodeLenCodes = 19;

struct BitReader {
    const std::uint8_t* data = nullptr;
    std::size_t size = 0;
    std::size_t byte_pos = 0;
    int bit_pos = 0; // next bit within data[byte_pos], LSB first
    bool overrun = false;

    [[nodiscard]] int bit() {
        if (byte_pos >= size) {
            overrun = true;
            return 0;
        }
        const int value = (data[byte_pos] >> bit_pos) & 1;
        if (++bit_pos == 8) {
            bit_pos = 0;
            ++byte_pos;
        }
        return value;
    }

    [[nodiscard]] std::uint32_t bits(int count) {
        std::uint32_t value = 0;
        for (int i = 0; i < count; ++i) {
            value |= static_cast<std::uint32_t>(bit()) << i;
        }
        return value;
    }

    void align_to_byte() {
        if (bit_pos != 0) {
            bit_pos = 0;
            ++byte_pos;
        }
    }
};

// Canonical Huffman table: count of codes per length + symbols sorted by
// (length, symbol). Decoding walks lengths accumulating the canonical offsets.
struct Huffman {
    std::array<int, kMaxBits + 1> count {};
    std::array<int, kMaxLitLenCodes> symbol {};
    bool valid = false;
    bool complete = false;

    void build(const int* lengths, int n) {
        count.fill(0);
        for (int i = 0; i < n; ++i) {
            ++count[static_cast<std::size_t>(lengths[i])];
        }
        valid = true;
        if (count[0] == n) {
            complete = false; // no codes at all
            valid = false;
            return;
        }
        int left = 1; // number of possible codes left at current length
        for (int len = 1; len <= kMaxBits; ++len) {
            left <<= 1;
            left -= count[static_cast<std::size_t>(len)];
            if (left < 0) {
                valid = false; // over-subscribed
                return;
            }
        }
        complete = (left == 0);
        std::array<int, kMaxBits + 1> offsets {};
        for (int len = 1; len < kMaxBits; ++len) {
            offsets[static_cast<std::size_t>(len + 1)] =
                offsets[static_cast<std::size_t>(len)] + count[static_cast<std::size_t>(len)];
        }
        for (int sym = 0; sym < n; ++sym) {
            if (lengths[sym] != 0) {
                symbol[static_cast<std::size_t>(offsets[static_cast<std::size_t>(lengths[sym])]++)] = sym;
            }
        }
    }

    // Decode one symbol; returns -1 on invalid code / input overrun.
    [[nodiscard]] int decode(BitReader& reader) const {
        int code = 0;
        int first = 0;
        int index = 0;
        for (int len = 1; len <= kMaxBits; ++len) {
            code |= reader.bit();
            if (reader.overrun) {
                return -1;
            }
            const int n = count[static_cast<std::size_t>(len)];
            if (code - first < n) {
                return symbol[static_cast<std::size_t>(index + (code - first))];
            }
            index += n;
            first += n;
            first <<= 1;
            code <<= 1;
        }
        return -1;
    }
};

constexpr std::array<int, 29> kLengthBase = {
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31,
    35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258,
};
constexpr std::array<int, 29> kLengthExtra = {
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2,
    3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
};
constexpr std::array<int, 30> kDistBase = {
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193,
    257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
};
constexpr std::array<int, 30> kDistExtra = {
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6,
    7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13,
};

bool inflate_codes(
    BitReader& reader,
    const Huffman& litlen,
    const Huffman& dist,
    std::vector<std::uint8_t>& out,
    std::string& error) {
    while (true) {
        int sym = litlen.decode(reader);
        if (sym < 0) {
            error = "inflate_bad_huffman_code";
            return false;
        }
        if (sym < 256) {
            out.push_back(static_cast<std::uint8_t>(sym));
            continue;
        }
        if (sym == 256) {
            return true; // end of block
        }
        sym -= 257;
        if (sym >= static_cast<int>(kLengthBase.size())) {
            error = "inflate_bad_length_symbol";
            return false;
        }
        const int length = kLengthBase[static_cast<std::size_t>(sym)] +
            static_cast<int>(reader.bits(kLengthExtra[static_cast<std::size_t>(sym)]));
        const int dist_sym = dist.decode(reader);
        if (dist_sym < 0 || dist_sym >= static_cast<int>(kDistBase.size())) {
            error = "inflate_bad_distance_symbol";
            return false;
        }
        const int distance = kDistBase[static_cast<std::size_t>(dist_sym)] +
            static_cast<int>(reader.bits(kDistExtra[static_cast<std::size_t>(dist_sym)]));
        if (reader.overrun) {
            error = "inflate_truncated_input";
            return false;
        }
        if (distance <= 0 || static_cast<std::size_t>(distance) > out.size()) {
            error = "inflate_distance_too_far";
            return false;
        }
        const std::size_t start = out.size() - static_cast<std::size_t>(distance);
        for (int i = 0; i < length; ++i) {
            out.push_back(out[start + static_cast<std::size_t>(i)]);
        }
    }
}

void build_fixed_tables(Huffman& litlen, Huffman& dist) {
    std::array<int, kMaxLitLenCodes> litlen_lengths {};
    for (int i = 0; i < 144; ++i) litlen_lengths[static_cast<std::size_t>(i)] = 8;
    for (int i = 144; i < 256; ++i) litlen_lengths[static_cast<std::size_t>(i)] = 9;
    for (int i = 256; i < 280; ++i) litlen_lengths[static_cast<std::size_t>(i)] = 7;
    for (int i = 280; i < 288; ++i) litlen_lengths[static_cast<std::size_t>(i)] = 8;
    litlen.build(litlen_lengths.data(), kMaxLitLenCodes);

    std::array<int, kMaxDistCodes> dist_lengths {};
    dist_lengths.fill(5);
    dist.build(dist_lengths.data(), kMaxDistCodes);
    // The fixed distance table is intentionally incomplete per RFC 1951.
    dist.valid = true;
}

bool inflate_dynamic_block(BitReader& reader, std::vector<std::uint8_t>& out, std::string& error) {
    const int hlit = static_cast<int>(reader.bits(5)) + 257;
    const int hdist = static_cast<int>(reader.bits(5)) + 1;
    const int hclen = static_cast<int>(reader.bits(4)) + 4;
    if (reader.overrun || hlit > kMaxLitLenCodes || hdist > kMaxDistCodes) {
        error = "inflate_bad_dynamic_header";
        return false;
    }

    constexpr std::array<int, kMaxCodeLenCodes> kOrder = {
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    };
    std::array<int, kMaxCodeLenCodes> codelen_lengths {};
    for (int i = 0; i < hclen; ++i) {
        codelen_lengths[static_cast<std::size_t>(kOrder[static_cast<std::size_t>(i)])] =
            static_cast<int>(reader.bits(3));
    }
    Huffman codelen;
    codelen.build(codelen_lengths.data(), kMaxCodeLenCodes);
    if (!codelen.valid || !codelen.complete) {
        error = "inflate_bad_codelen_table";
        return false;
    }

    std::array<int, kMaxLitLenCodes + kMaxDistCodes> lengths {};
    int index = 0;
    while (index < hlit + hdist) {
        const int sym = codelen.decode(reader);
        if (sym < 0) {
            error = "inflate_bad_codelen_symbol";
            return false;
        }
        if (sym < 16) {
            lengths[static_cast<std::size_t>(index++)] = sym;
            continue;
        }
        int repeat = 0;
        int value = 0;
        if (sym == 16) {
            if (index == 0) {
                error = "inflate_repeat_without_prior_length";
                return false;
            }
            value = lengths[static_cast<std::size_t>(index - 1)];
            repeat = 3 + static_cast<int>(reader.bits(2));
        } else if (sym == 17) {
            repeat = 3 + static_cast<int>(reader.bits(3));
        } else {
            repeat = 11 + static_cast<int>(reader.bits(7));
        }
        if (reader.overrun || index + repeat > hlit + hdist) {
            error = "inflate_repeat_overflow";
            return false;
        }
        while (repeat-- > 0) {
            lengths[static_cast<std::size_t>(index++)] = value;
        }
    }
    if (lengths[256] == 0) {
        error = "inflate_missing_end_of_block_code";
        return false;
    }

    Huffman litlen;
    litlen.build(lengths.data(), hlit);
    Huffman dist;
    dist.build(lengths.data() + hlit, hdist);
    // A single-distance-code table may be incomplete; accept as zlib does when
    // it is not over-subscribed.
    if (!litlen.valid) {
        error = "inflate_bad_litlen_table";
        return false;
    }
    if (!dist.valid && dist.count[0] != hdist) {
        error = "inflate_bad_distance_table";
        return false;
    }
    return inflate_codes(reader, litlen, dist, out, error);
}

// ---------------------------------------------------------------------------
// PNG scanline unfiltering.
// ---------------------------------------------------------------------------

std::uint8_t paeth_predictor(int a, int b, int c) {
    const int p = a + b - c;
    const int pa = std::abs(p - a);
    const int pb = std::abs(p - b);
    const int pc = std::abs(p - c);
    if (pa <= pb && pa <= pc) {
        return static_cast<std::uint8_t>(a);
    }
    if (pb <= pc) {
        return static_cast<std::uint8_t>(b);
    }
    return static_cast<std::uint8_t>(c);
}

std::uint32_t read_be32(const std::uint8_t* p) {
    return (static_cast<std::uint32_t>(p[0]) << 24) | (static_cast<std::uint32_t>(p[1]) << 16) |
        (static_cast<std::uint32_t>(p[2]) << 8) | static_cast<std::uint32_t>(p[3]);
}

} // namespace

InflateResult inflate_deflate_stream(const std::uint8_t* data, std::size_t size) {
    InflateResult result;
    BitReader reader;
    reader.data = data;
    reader.size = size;

    bool final_block = false;
    while (!final_block) {
        final_block = reader.bit() != 0;
        const std::uint32_t block_type = reader.bits(2);
        if (reader.overrun) {
            result.error = "inflate_truncated_input";
            return result;
        }
        if (block_type == 0) {
            reader.align_to_byte();
            if (reader.byte_pos + 4 > reader.size) {
                result.error = "inflate_truncated_stored_header";
                return result;
            }
            const std::uint32_t len = static_cast<std::uint32_t>(data[reader.byte_pos]) |
                (static_cast<std::uint32_t>(data[reader.byte_pos + 1]) << 8);
            const std::uint32_t nlen = static_cast<std::uint32_t>(data[reader.byte_pos + 2]) |
                (static_cast<std::uint32_t>(data[reader.byte_pos + 3]) << 8);
            if ((len ^ 0xFFFFu) != nlen) {
                result.error = "inflate_stored_length_mismatch";
                return result;
            }
            reader.byte_pos += 4;
            if (reader.byte_pos + len > reader.size) {
                result.error = "inflate_truncated_stored_data";
                return result;
            }
            result.bytes.insert(result.bytes.end(), data + reader.byte_pos, data + reader.byte_pos + len);
            reader.byte_pos += len;
        } else if (block_type == 1) {
            Huffman litlen;
            Huffman dist;
            build_fixed_tables(litlen, dist);
            if (!inflate_codes(reader, litlen, dist, result.bytes, result.error)) {
                return result;
            }
        } else if (block_type == 2) {
            if (!inflate_dynamic_block(reader, result.bytes, result.error)) {
                return result;
            }
        } else {
            result.error = "inflate_reserved_block_type";
            return result;
        }
    }
    result.ok = true;
    return result;
}

InflateResult inflate_zlib_stream(const std::uint8_t* data, std::size_t size) {
    InflateResult result;
    if (size < 6) {
        result.error = "zlib_stream_too_short";
        return result;
    }
    const std::uint8_t cmf = data[0];
    const std::uint8_t flg = data[1];
    if ((cmf & 0x0F) != 8) {
        result.error = "zlib_unsupported_method";
        return result;
    }
    if (((static_cast<unsigned>(cmf) << 8) | flg) % 31 != 0) {
        result.error = "zlib_bad_header_check";
        return result;
    }
    if ((flg & 0x20) != 0) {
        result.error = "zlib_preset_dictionary_unsupported";
        return result;
    }
    result = inflate_deflate_stream(data + 2, size - 2);
    if (!result.ok) {
        return result;
    }
    // Verify Adler-32 trailer over the decompressed bytes.
    std::uint32_t s1 = 1;
    std::uint32_t s2 = 0;
    for (const std::uint8_t byte : result.bytes) {
        s1 = (s1 + byte) % 65521u;
        s2 = (s2 + s1) % 65521u;
    }
    const std::uint32_t expected = read_be32(data + size - 4);
    if (((s2 << 16) | s1) != expected) {
        result.ok = false;
        result.error = "zlib_adler32_mismatch";
        result.bytes.clear();
    }
    return result;
}

PngImage decode_png_rgba(const std::vector<std::uint8_t>& file_bytes) {
    PngImage image;
    static constexpr std::array<std::uint8_t, 8> kSignature = {
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
    };
    if (file_bytes.size() < 8 ||
        !std::equal(kSignature.begin(), kSignature.end(), file_bytes.begin())) {
        image.error = "png_bad_signature";
        return image;
    }

    int bit_depth = 0;
    int color_type = -1;
    std::vector<std::uint8_t> palette;      // PLTE: packed RGB triples
    std::vector<std::uint8_t> palette_alpha; // tRNS: per-entry alpha
    std::vector<std::uint8_t> compressed;
    std::size_t pos = 8;
    bool saw_ihdr = false;
    while (pos + 8 <= file_bytes.size()) {
        const std::uint32_t length = read_be32(file_bytes.data() + pos);
        if (pos + 12 + length > file_bytes.size()) {
            image.error = "png_truncated_chunk";
            return image;
        }
        const char* type = reinterpret_cast<const char*>(file_bytes.data() + pos + 4);
        const std::uint8_t* payload = file_bytes.data() + pos + 8;
        if (std::memcmp(type, "IHDR", 4) == 0) {
            if (length != 13) {
                image.error = "png_bad_ihdr";
                return image;
            }
            image.width = static_cast<int>(read_be32(payload));
            image.height = static_cast<int>(read_be32(payload + 4));
            bit_depth = payload[8];
            color_type = payload[9];
            const int interlace = payload[12];
            if (image.width <= 0 || image.height <= 0 || image.width > 1 << 15 ||
                image.height > 1 << 15) {
                image.error = "png_bad_dimensions";
                return image;
            }
            if (bit_depth != 8) {
                image.error = "png_unsupported_bit_depth";
                return image;
            }
            if (color_type != 0 && color_type != 2 && color_type != 3 &&
                color_type != 4 && color_type != 6) {
                image.error = "png_unsupported_color_type";
                return image;
            }
            if (interlace != 0) {
                image.error = "png_interlace_unsupported";
                return image;
            }
            saw_ihdr = true;
        } else if (std::memcmp(type, "PLTE", 4) == 0) {
            if (length == 0 || length % 3 != 0 || length > 256 * 3) {
                image.error = "png_bad_plte";
                return image;
            }
            palette.assign(payload, payload + length);
        } else if (std::memcmp(type, "tRNS", 4) == 0 && color_type == 3) {
            if (length > 256) {
                image.error = "png_bad_trns";
                return image;
            }
            palette_alpha.assign(payload, payload + length);
        } else if (std::memcmp(type, "IDAT", 4) == 0) {
            compressed.insert(compressed.end(), payload, payload + length);
        } else if (std::memcmp(type, "IEND", 4) == 0) {
            break;
        }
        pos += 12 + length;
    }
    if (!saw_ihdr || compressed.empty()) {
        image.error = saw_ihdr ? "png_missing_idat" : "png_missing_ihdr";
        return image;
    }
    if (color_type == 3 && palette.empty()) {
        image.error = "png_missing_plte";
        return image;
    }

    const InflateResult inflated = inflate_zlib_stream(compressed.data(), compressed.size());
    if (!inflated.ok) {
        image.error = inflated.error;
        return image;
    }

    const int channels = (color_type == 0 || color_type == 3) ? 1
        : color_type == 2 ? 3 : color_type == 4 ? 2 : 4;
    const std::size_t stride =
        static_cast<std::size_t>(image.width) * static_cast<std::size_t>(channels);
    const std::size_t expected = (stride + 1) * static_cast<std::size_t>(image.height);
    if (inflated.bytes.size() != expected) {
        image.error = "png_scanline_size_mismatch";
        return image;
    }

    std::vector<std::uint8_t> raw(stride * static_cast<std::size_t>(image.height));
    for (int row = 0; row < image.height; ++row) {
        const std::uint8_t* src = inflated.bytes.data() + static_cast<std::size_t>(row) * (stride + 1);
        const std::uint8_t filter = src[0];
        std::uint8_t* dst = raw.data() + static_cast<std::size_t>(row) * stride;
        const std::uint8_t* prior = row > 0 ? dst - stride : nullptr;
        for (std::size_t i = 0; i < stride; ++i) {
            const int a = i >= static_cast<std::size_t>(channels)
                ? dst[i - static_cast<std::size_t>(channels)] : 0;
            const int b = prior != nullptr ? prior[i] : 0;
            const int c = (prior != nullptr && i >= static_cast<std::size_t>(channels))
                ? prior[i - static_cast<std::size_t>(channels)] : 0;
            const int x = src[1 + i];
            std::uint8_t value = 0;
            switch (filter) {
            case 0: value = static_cast<std::uint8_t>(x); break;
            case 1: value = static_cast<std::uint8_t>(x + a); break;
            case 2: value = static_cast<std::uint8_t>(x + b); break;
            case 3: value = static_cast<std::uint8_t>(x + (a + b) / 2); break;
            case 4: value = static_cast<std::uint8_t>(x + paeth_predictor(a, b, c)); break;
            default:
                image.error = "png_unknown_filter";
                return image;
            }
            dst[i] = value;
        }
    }

    image.rgba.resize(static_cast<std::size_t>(image.width) *
                      static_cast<std::size_t>(image.height) * 4);
    const std::size_t pixel_count =
        static_cast<std::size_t>(image.width) * static_cast<std::size_t>(image.height);
    for (std::size_t i = 0; i < pixel_count; ++i) {
        const std::uint8_t* src = raw.data() + i * static_cast<std::size_t>(channels);
        std::uint8_t* dst = image.rgba.data() + i * 4;
        switch (color_type) {
        case 0: dst[0] = dst[1] = dst[2] = src[0]; dst[3] = 255; break;
        case 2: dst[0] = src[0]; dst[1] = src[1]; dst[2] = src[2]; dst[3] = 255; break;
        case 3: {
            const std::size_t index = src[0];
            if (index * 3 + 2 >= palette.size()) {
                image.error = "png_palette_index_out_of_range";
                return image;
            }
            dst[0] = palette[index * 3];
            dst[1] = palette[index * 3 + 1];
            dst[2] = palette[index * 3 + 2];
            dst[3] = index < palette_alpha.size() ? palette_alpha[index] : 255;
            break;
        }
        case 4: dst[0] = dst[1] = dst[2] = src[0]; dst[3] = src[1]; break;
        default: dst[0] = src[0]; dst[1] = src[1]; dst[2] = src[2]; dst[3] = src[3]; break;
        }
    }
    image.ok = true;
    return image;
}

PngImage decode_png_rgba_file(const std::filesystem::path& path) {
    std::ifstream stream(path, std::ios::binary);
    if (!stream) {
        PngImage image;
        image.error = "png_file_open_failed";
        return image;
    }
    std::vector<std::uint8_t> bytes(
        (std::istreambuf_iterator<char>(stream)), std::istreambuf_iterator<char>());
    return decode_png_rgba(bytes);
}

} // namespace agbot::terrain
