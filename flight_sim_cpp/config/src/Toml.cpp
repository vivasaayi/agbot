#include "agbot_config/Toml.hpp"

#include <cctype>
#include <charconv>
#include <fstream>
#include <sstream>

namespace agbot::config {

namespace {

struct Cursor {
    const std::string& text;
    std::size_t position = 0;
    int line = 1;

    bool done() const { return position >= text.size(); }
    char peek() const { return done() ? '\0' : text[position]; }
    char advance() {
        const char character = peek();
        ++position;
        if (character == '\n') {
            ++line;
        }
        return character;
    }
};

struct ParseError {
    std::string message;
};

void skip_inline_whitespace(Cursor& cursor) {
    while (!cursor.done() && (cursor.peek() == ' ' || cursor.peek() == '\t')) {
        cursor.advance();
    }
}

void skip_to_line_end(Cursor& cursor) {
    while (!cursor.done() && cursor.peek() != '\n') {
        cursor.advance();
    }
}

[[noreturn]] void fail(const Cursor& cursor, const std::string& message) {
    throw ParseError{"line " + std::to_string(cursor.line) + ": " + message};
}

bool is_bare_key_char(char character) {
    return std::isalnum(static_cast<unsigned char>(character)) != 0 || character == '_' ||
        character == '-';
}

std::string parse_bare_or_quoted_key(Cursor& cursor) {
    skip_inline_whitespace(cursor);
    if (cursor.peek() == '"') {
        cursor.advance();
        std::string key;
        while (!cursor.done() && cursor.peek() != '"') {
            key.push_back(cursor.advance());
        }
        if (cursor.done()) {
            fail(cursor, "unterminated quoted key");
        }
        cursor.advance();
        return key;
    }
    std::string key;
    while (!cursor.done() && is_bare_key_char(cursor.peek())) {
        key.push_back(cursor.advance());
    }
    if (key.empty()) {
        fail(cursor, "expected key");
    }
    return key;
}

std::string parse_basic_string(Cursor& cursor) {
    // Assumes opening quote already consumed.
    std::string value;
    while (true) {
        if (cursor.done() || cursor.peek() == '\n') {
            fail(cursor, "unterminated string");
        }
        const char character = cursor.advance();
        if (character == '"') {
            break;
        }
        if (character == '\\') {
            if (cursor.done()) {
                fail(cursor, "unterminated escape sequence");
            }
            const char escaped = cursor.advance();
            switch (escaped) {
            case 'n': value.push_back('\n'); break;
            case 't': value.push_back('\t'); break;
            case 'r': value.push_back('\r'); break;
            case '"': value.push_back('"'); break;
            case '\\': value.push_back('\\'); break;
            default:
                fail(cursor, std::string("unsupported escape \\") + escaped);
            }
            continue;
        }
        value.push_back(character);
    }
    return value;
}

ParamValue parse_value(Cursor& cursor);

ParamValue parse_number_or_bool(Cursor& cursor) {
    std::string token;
    while (!cursor.done()) {
        const char character = cursor.peek();
        if (std::isalnum(static_cast<unsigned char>(character)) != 0 || character == '+' ||
            character == '-' || character == '.' || character == '_') {
            token.push_back(cursor.advance());
        } else {
            break;
        }
    }
    if (token == "true") {
        return ParamValue(true);
    }
    if (token == "false") {
        return ParamValue(false);
    }
    std::string cleaned;
    cleaned.reserve(token.size());
    for (const char character : token) {
        if (character != '_') {
            cleaned.push_back(character);
        }
    }
    const bool looks_float = cleaned.find('.') != std::string::npos ||
        cleaned.find('e') != std::string::npos || cleaned.find('E') != std::string::npos;
    if (!looks_float) {
        std::int64_t integer_value = 0;
        const auto [ptr, ec] =
            std::from_chars(cleaned.data(), cleaned.data() + cleaned.size(), integer_value);
        if (ec == std::errc() && ptr == cleaned.data() + cleaned.size()) {
            return ParamValue(integer_value);
        }
    }
    try {
        std::size_t consumed = 0;
        const double double_value = std::stod(cleaned, &consumed);
        if (consumed == cleaned.size()) {
            return ParamValue(double_value);
        }
    } catch (...) {
    }
    fail(cursor, "invalid value token '" + token + "'");
}

ParamValue parse_array(Cursor& cursor) {
    // Assumes '[' already consumed.
    ParamArray array;
    while (true) {
        skip_inline_whitespace(cursor);
        while (cursor.peek() == '\n' || cursor.peek() == '#') {
            if (cursor.peek() == '#') {
                skip_to_line_end(cursor);
            } else {
                cursor.advance();
            }
            skip_inline_whitespace(cursor);
        }
        if (cursor.peek() == ']') {
            cursor.advance();
            break;
        }
        array.push_back(parse_value(cursor));
        skip_inline_whitespace(cursor);
        while (cursor.peek() == '\n') {
            cursor.advance();
            skip_inline_whitespace(cursor);
        }
        if (cursor.peek() == ',') {
            cursor.advance();
            continue;
        }
        if (cursor.peek() == ']') {
            cursor.advance();
            break;
        }
        fail(cursor, "expected ',' or ']' in array");
    }
    return ParamValue(std::move(array));
}

ParamValue parse_inline_table(Cursor& cursor) {
    // Assumes '{' already consumed.
    ParamTable table;
    skip_inline_whitespace(cursor);
    if (cursor.peek() == '}') {
        cursor.advance();
        return ParamValue(std::move(table));
    }
    while (true) {
        const std::string key = parse_bare_or_quoted_key(cursor);
        skip_inline_whitespace(cursor);
        if (cursor.peek() != '=') {
            fail(cursor, "expected '=' in inline table");
        }
        cursor.advance();
        skip_inline_whitespace(cursor);
        table[key] = parse_value(cursor);
        skip_inline_whitespace(cursor);
        if (cursor.peek() == ',') {
            cursor.advance();
            skip_inline_whitespace(cursor);
            continue;
        }
        if (cursor.peek() == '}') {
            cursor.advance();
            break;
        }
        fail(cursor, "expected ',' or '}' in inline table");
    }
    return ParamValue(std::move(table));
}

ParamValue parse_value(Cursor& cursor) {
    skip_inline_whitespace(cursor);
    const char character = cursor.peek();
    if (character == '"') {
        cursor.advance();
        return ParamValue(parse_basic_string(cursor));
    }
    if (character == '[') {
        cursor.advance();
        return parse_array(cursor);
    }
    if (character == '{') {
        cursor.advance();
        return parse_inline_table(cursor);
    }
    return parse_number_or_bool(cursor);
}

std::vector<std::string> parse_header_path(Cursor& cursor) {
    std::vector<std::string> path;
    while (true) {
        path.push_back(parse_bare_or_quoted_key(cursor));
        skip_inline_whitespace(cursor);
        if (cursor.peek() == '.') {
            cursor.advance();
            continue;
        }
        break;
    }
    return path;
}

ParamTable* descend(ParamTable& root, const std::vector<std::string>& path, Cursor& cursor,
                    bool array_of_tables) {
    ParamTable* current = &root;
    for (std::size_t index = 0; index < path.size(); ++index) {
        const std::string& key = path[index];
        const bool leaf = index + 1 == path.size();
        auto it = current->find(key);
        if (leaf && array_of_tables) {
            if (it == current->end()) {
                it = current->emplace(key, ParamValue(ParamArray{})).first;
            }
            if (!it->second.is_array()) {
                fail(cursor, "key '" + key + "' is not an array of tables");
            }
            ParamArray array = it->second.as_array();
            array.push_back(ParamValue(ParamTable{}));
            it->second = ParamValue(std::move(array));
            ParamArray& stored = const_cast<ParamArray&>(it->second.as_array());
            return &const_cast<ParamTable&>(stored.back().as_table());
        }
        if (it == current->end()) {
            it = current->emplace(key, ParamValue(ParamTable{})).first;
        }
        if (it->second.is_array()) {
            // Descend into the last element of an array-of-tables.
            ParamArray& array = const_cast<ParamArray&>(it->second.as_array());
            if (array.empty() || !array.back().is_table()) {
                fail(cursor, "cannot descend into array '" + key + "'");
            }
            current = &const_cast<ParamTable&>(array.back().as_table());
            continue;
        }
        if (!it->second.is_table()) {
            fail(cursor, "key '" + key + "' is not a table");
        }
        current = &it->second.as_table();
    }
    return current;
}

} // namespace

TomlParseResult parse_toml(const std::string& text) {
    TomlParseResult result;
    Cursor cursor{text};
    ParamTable* current_table = &result.root;
    try {
        while (!cursor.done()) {
            skip_inline_whitespace(cursor);
            const char character = cursor.peek();
            if (character == '\0') {
                break;
            }
            if (character == '\n') {
                cursor.advance();
                continue;
            }
            if (character == '#') {
                skip_to_line_end(cursor);
                continue;
            }
            if (character == '[') {
                cursor.advance();
                const bool array_of_tables = cursor.peek() == '[';
                if (array_of_tables) {
                    cursor.advance();
                }
                const std::vector<std::string> path = parse_header_path(cursor);
                if (cursor.peek() != ']') {
                    fail(cursor, "expected ']' in table header");
                }
                cursor.advance();
                if (array_of_tables) {
                    if (cursor.peek() != ']') {
                        fail(cursor, "expected ']]' in array-of-tables header");
                    }
                    cursor.advance();
                }
                current_table = descend(result.root, path, cursor, array_of_tables);
                skip_inline_whitespace(cursor);
                if (cursor.peek() == '#') {
                    skip_to_line_end(cursor);
                }
                continue;
            }
            const std::string key = parse_bare_or_quoted_key(cursor);
            skip_inline_whitespace(cursor);
            if (cursor.peek() != '=') {
                fail(cursor, "expected '=' after key '" + key + "'");
            }
            cursor.advance();
            (*current_table)[key] = parse_value(cursor);
            skip_inline_whitespace(cursor);
            if (cursor.peek() == '#') {
                skip_to_line_end(cursor);
            }
            if (!cursor.done() && cursor.peek() != '\n') {
                fail(cursor, "unexpected trailing content after value for key '" + key + "'");
            }
        }
        result.ok = true;
    } catch (const ParseError& error) {
        result.ok = false;
        result.error = error.message;
        result.root.clear();
    }
    return result;
}

TomlParseResult parse_toml_file(const std::filesystem::path& path) {
    std::ifstream file(path);
    if (!file) {
        TomlParseResult result;
        result.error = "cannot open config file: " + path.string();
        return result;
    }
    std::ostringstream buffer;
    buffer << file.rdbuf();
    return parse_toml(buffer.str());
}

} // namespace agbot::config
