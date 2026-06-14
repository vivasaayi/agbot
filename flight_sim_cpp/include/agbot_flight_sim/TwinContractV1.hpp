#pragma once

#include <cstdint>
#include <string>
#include <string_view>
#include <vector>

namespace agbot::flight_sim {

struct ContractTypeV1 {
    std::string name;
    std::vector<std::string> required_fields;
};

struct TwinContractSchemaV1 {
    std::string name;
    std::string version;
    std::vector<ContractTypeV1> types;
    std::vector<std::string> capabilities;
    std::string schema_hash;

    [[nodiscard]] bool has_type(std::string_view type_name) const;
    [[nodiscard]] bool type_has_field(std::string_view type_name, std::string_view field_name) const;
    [[nodiscard]] bool has_capability(std::string_view capability) const;
    [[nodiscard]] std::string to_json() const;
};

[[nodiscard]] std::string sha256_hex(std::string_view bytes);
[[nodiscard]] const TwinContractSchemaV1& twin_contract_v1_schema();
[[nodiscard]] bool is_compatible_contract_version(
    std::string_view consumer_version,
    std::string_view producer_version);

} // namespace agbot::flight_sim
