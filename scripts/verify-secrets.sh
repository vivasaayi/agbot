#!/usr/bin/env bash
set -euo pipefail

failures=0

trim() {
    local value="$1"
    value="${value#"${value%%[![:space:]]*}"}"
    value="${value%"${value##*[![:space:]]}"}"
    value="${value%\"}"
    value="${value#\"}"
    value="${value%\'}"
    value="${value#\'}"
    printf '%s' "$value"
}

is_managed_reference() {
    local value lower_value
    value="$(trim "$1")"
    lower_value="$(printf '%s' "$value" | tr '[:upper:]' '[:lower:]')"
    [[ -z "$value" ]] && return 0
    [[ "$value" == \${* ]] && return 0
    [[ "$value" == \$* ]] && return 0
    [[ "$value" == /run/secrets/* ]] && return 0
    [[ "$lower_value" == "null" ]] && return 0
    return 1
}

scan_file() {
    local file="$1"
    [[ -f "$file" ]] || return 0

    local line_number=0
    while IFS= read -r raw_line || [[ -n "$raw_line" ]]; do
        line_number=$((line_number + 1))
        local line key value key_upper
        line="$(trim "$raw_line")"
        [[ -z "$line" || "$line" == \#* ]] && continue
        [[ "$line" == "- "* ]] && line="$(trim "${line#- }")"
        [[ "$line" == ENV\ * ]] && line="$(trim "${line#ENV }")"

        if [[ "$line" == *"="* ]]; then
            key="$(trim "${line%%=*}")"
            value="$(trim "${line#*=}")"
        elif [[ "$line" == *":"* ]]; then
            key="$(trim "${line%%:*}")"
            value="$(trim "${line#*:}")"
        else
            continue
        fi

        [[ -z "$key" || -z "$value" ]] && continue
        key_upper="$(printf '%s' "$key" | tr '[:lower:]' '[:upper:]')"
        [[ "$key_upper" == *_FILE ]] && continue
        [[ ! "$key_upper" =~ (PASSWORD|TOKEN|SECRET|API_KEY|DATABASE_URL) ]] && continue

        if ! is_managed_reference "$value"; then
            echo "secret-scan: $file:$line_number has plaintext secret-like value for $key" >&2
            failures=$((failures + 1))
        fi
    done < "$file"
}

if (( $# > 0 )); then
    files=("$@")
else
    files=(docker-compose.yml .env Dockerfile)
fi

for file in "${files[@]}"; do
    scan_file "$file"
done

if (( failures > 0 )); then
    exit 1
fi

echo "secret-scan: no plaintext committed secrets detected"
