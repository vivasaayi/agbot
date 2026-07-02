#pragma once

#include "agbot_config/Params.hpp"

#include <functional>
#include <map>
#include <memory>
#include <string>
#include <vector>

namespace agbot::vehicles {

// StrategyRegistry variant whose factories build strategies from a ParamTable,
// so every algorithm is hot-swappable and fully parameterized from config.
template <typename Interface>
class ParamRegistry {
public:
    using Factory = std::function<std::unique_ptr<Interface>(const agbot::config::ParamTable&)>;

    void register_factory(const std::string& name, Factory factory) {
        factories_[name] = std::move(factory);
    }

    [[nodiscard]] bool contains(const std::string& name) const {
        return factories_.count(name) > 0;
    }

    [[nodiscard]] std::unique_ptr<Interface> create(
        const std::string& name,
        const agbot::config::ParamTable& params) const {
        const auto it = factories_.find(name);
        if (it == factories_.end()) {
            return nullptr;
        }
        return it->second(params);
    }

    [[nodiscard]] std::vector<std::string> available() const {
        std::vector<std::string> names;
        names.reserve(factories_.size());
        for (const auto& [name, factory] : factories_) {
            names.push_back(name);
        }
        return names;
    }

private:
    std::map<std::string, Factory> factories_;
};

} // namespace agbot::vehicles
