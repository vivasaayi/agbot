#include "agbot_config/Params.hpp"

#include <cinttypes>
#include <cstdio>
#include <sstream>

namespace agbot::config {

namespace {

constexpr std::uint64_t kFnvOffsetBasis = 1469598103934665603ULL;
constexpr std::uint64_t kFnvPrime = 1099511628211ULL;

std::uint64_t fnv1a(const std::string& text) {
    std::uint64_t hash = kFnvOffsetBasis;
    for (const char character : text) {
        hash ^= static_cast<std::uint64_t>(static_cast<unsigned char>(character));
        hash *= kFnvPrime;
    }
    return hash;
}

std::string format_double(double value) {
    char buffer[64];
    std::snprintf(buffer, sizeof(buffer), "%.17g", value);
    return buffer;
}

} // namespace

const ParamValue* find(const ParamTable& table, const std::string& key) {
    const auto it = table.find(key);
    if (it == table.end()) {
        return nullptr;
    }
    return &it->second;
}

const ParamTable* find_table(const ParamTable& table, const std::string& key) {
    const ParamValue* value = find(table, key);
    if (value == nullptr || !value->is_table()) {
        return nullptr;
    }
    return &value->as_table();
}

const ParamArray* find_array(const ParamTable& table, const std::string& key) {
    const ParamValue* value = find(table, key);
    if (value == nullptr || !value->is_array()) {
        return nullptr;
    }
    return &value->as_array();
}

double double_or(const ParamTable& table, const std::string& key, double fallback) {
    const ParamValue* value = find(table, key);
    if (value == nullptr || !value->is_number()) {
        return fallback;
    }
    return value->as_double();
}

std::int64_t integer_or(const ParamTable& table, const std::string& key, std::int64_t fallback) {
    const ParamValue* value = find(table, key);
    if (value == nullptr || !value->is_integer()) {
        return fallback;
    }
    return value->as_integer();
}

bool bool_or(const ParamTable& table, const std::string& key, bool fallback) {
    const ParamValue* value = find(table, key);
    if (value == nullptr || !value->is_bool()) {
        return fallback;
    }
    return value->as_bool();
}

std::string string_or(const ParamTable& table, const std::string& key, const std::string& fallback) {
    const ParamValue* value = find(table, key);
    if (value == nullptr || !value->is_string()) {
        return fallback;
    }
    return value->as_string();
}

std::string canonical_string(const ParamValue& value) {
    std::ostringstream out;
    if (value.is_bool()) {
        out << (value.as_bool() ? "b:true" : "b:false");
    } else if (value.is_integer()) {
        out << "i:" << value.as_integer();
    } else if (value.is_double()) {
        out << "d:" << format_double(value.as_double());
    } else if (value.is_string()) {
        out << "s:" << value.as_string();
    } else if (value.is_array()) {
        out << "a:[";
        for (const ParamValue& element : value.as_array()) {
            out << canonical_string(element) << ",";
        }
        out << "]";
    } else if (value.is_table()) {
        out << "t:" << canonical_string(value.as_table());
    }
    return out.str();
}

std::string canonical_string(const ParamTable& table) {
    // std::map iterates in sorted key order, so serialization is canonical.
    std::ostringstream out;
    out << "{";
    for (const auto& [key, value] : table) {
        out << key << "=" << canonical_string(value) << ";";
    }
    out << "}";
    return out.str();
}

std::uint64_t param_hash(const ParamTable& table) {
    return fnv1a(canonical_string(table));
}

} // namespace agbot::config
