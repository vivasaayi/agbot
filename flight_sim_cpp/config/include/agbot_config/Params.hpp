#pragma once

#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <variant>
#include <vector>

namespace agbot::config {

class ParamValue;

using ParamArray = std::vector<ParamValue>;
using ParamTable = std::map<std::string, ParamValue>;

// A single configuration value: scalar, array, or nested table.
class ParamValue {
public:
    using Storage = std::variant<bool, std::int64_t, double, std::string, ParamArray, ParamTable>;

    ParamValue() : storage_(false) {}
    ParamValue(bool value) : storage_(value) {}
    ParamValue(std::int64_t value) : storage_(value) {}
    ParamValue(int value) : storage_(static_cast<std::int64_t>(value)) {}
    ParamValue(double value) : storage_(value) {}
    ParamValue(const char* value) : storage_(std::string(value)) {}
    ParamValue(std::string value) : storage_(std::move(value)) {}
    ParamValue(ParamArray value) : storage_(std::move(value)) {}
    ParamValue(ParamTable value) : storage_(std::move(value)) {}

    bool is_bool() const { return std::holds_alternative<bool>(storage_); }
    bool is_integer() const { return std::holds_alternative<std::int64_t>(storage_); }
    bool is_double() const { return std::holds_alternative<double>(storage_); }
    bool is_number() const { return is_integer() || is_double(); }
    bool is_string() const { return std::holds_alternative<std::string>(storage_); }
    bool is_array() const { return std::holds_alternative<ParamArray>(storage_); }
    bool is_table() const { return std::holds_alternative<ParamTable>(storage_); }

    bool as_bool() const { return std::get<bool>(storage_); }
    std::int64_t as_integer() const { return std::get<std::int64_t>(storage_); }
    double as_double() const {
        if (is_integer()) {
            return static_cast<double>(std::get<std::int64_t>(storage_));
        }
        return std::get<double>(storage_);
    }
    const std::string& as_string() const { return std::get<std::string>(storage_); }
    const ParamArray& as_array() const { return std::get<ParamArray>(storage_); }
    const ParamTable& as_table() const { return std::get<ParamTable>(storage_); }
    ParamTable& as_table() { return std::get<ParamTable>(storage_); }

    const Storage& storage() const { return storage_; }

private:
    Storage storage_;
};

// Convenience typed lookups with defaults over a ParamTable.
double double_or(const ParamTable& table, const std::string& key, double fallback);
std::int64_t integer_or(const ParamTable& table, const std::string& key, std::int64_t fallback);
bool bool_or(const ParamTable& table, const std::string& key, bool fallback);
std::string string_or(const ParamTable& table, const std::string& key, const std::string& fallback);
const ParamValue* find(const ParamTable& table, const std::string& key);
const ParamTable* find_table(const ParamTable& table, const std::string& key);
const ParamArray* find_array(const ParamTable& table, const std::string& key);

// Canonical serialization (sorted keys, stable float formatting) used for hashing.
std::string canonical_string(const ParamValue& value);
std::string canonical_string(const ParamTable& table);

// FNV1a-64 over the canonical serialization. Matches the repo's determinism idiom:
// identical configs hash identically across runs and platforms.
std::uint64_t param_hash(const ParamTable& table);

} // namespace agbot::config
