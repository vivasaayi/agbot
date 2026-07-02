#pragma once

#include <functional>
#include <map>
#include <memory>
#include <string>
#include <vector>

namespace agbot::config {

// Name -> factory registry so every subsystem's algorithms are hot-swappable
// from configuration. One registry instance per strategy interface.
template <typename Interface>
class StrategyRegistry {
public:
    using Factory = std::function<std::unique_ptr<Interface>()>;

    void register_factory(const std::string& name, Factory factory) {
        factories_[name] = std::move(factory);
    }

    bool contains(const std::string& name) const { return factories_.count(name) > 0; }

    std::unique_ptr<Interface> create(const std::string& name) const {
        const auto it = factories_.find(name);
        if (it == factories_.end()) {
            return nullptr;
        }
        return it->second();
    }

    std::vector<std::string> available() const {
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

} // namespace agbot::config
