#!/bin/sh
# Exercise the installed CLI through symlinks without a Docker daemon or network.
set -eu

test_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
test_root=${AGBOT_TEST_ROOT:-$test_root}
test_root=$(CDPATH= cd -- "$test_root" && pwd -P)
test_tmp=$(mktemp -d)
trap 'rm -rf "$test_tmp"' EXIT HUP INT TERM
mkdir -p "$test_tmp/bin" "$test_tmp/links" "$test_tmp/home"
ln -s "$test_root/scripts/agbot" "$test_tmp/links/absolute"
ln -s absolute "$test_tmp/links/relative"
ln -s ../links/relative "$test_tmp/bin/agbot"

for entry in "$test_tmp/links/absolute" "$test_tmp/links/relative" "$test_tmp/bin/agbot"; do
    actual=$(AGBOT_HOME="$test_tmp/home" sh "$entry" version)
    case "$actual" in
        "agbot CLI (repo $test_root)"*) ;;
        *) printf 'FAIL: incorrect repository through %s:\n%s\n' "$entry" "$actual" >&2; exit 1 ;;
    esac
done

cat > "$test_tmp/bin/docker" <<'EOF'
#!/bin/sh
set -eu
[ -n "$AGBOT_BOOTSTRAP_ACCESS_CODE" ] || exit 1
printf '%s' "$AGBOT_BOOTSTRAP_ACCESS_CODE" > "$TEST_CODE_LOG"
case "$*" in
    'compose version') exit 0 ;;
    'image inspect '*) printf 'example.invalid/agbot@sha256:fixture\n'; exit 0 ;;
esac
[ "$1" = compose ] && [ "$2" = -f ] || exit 1
[ "$3" = "$TEST_COMPOSE_FILE" ] && [ -f "$3" ] || exit 1
shift 3
if [ "${TEST_PULL_FAILURE:-false}" = true ] && [ "$1" = pull ]; then
    printf 'manifest unknown\n' >&2
    exit 1
fi
printf '%s\n' "$*" >> "$TEST_DOCKER_LOG"
EOF
cat > "$test_tmp/bin/curl" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >> "$TEST_CURL_LOG"
EOF
chmod +x "$test_tmp/bin/docker" "$test_tmp/bin/curl"
export TEST_COMPOSE_FILE="$test_root/docker-compose.prod.yml"
export TEST_DOCKER_LOG="$test_tmp/docker.log"
export TEST_CURL_LOG="$test_tmp/curl.log"
export TEST_CODE_LOG="$test_tmp/code.log"
PATH="$test_tmp/bin:$PATH" AGBOT_HOME="$test_tmp/home" sh "$test_tmp/bin/agbot" up
[ "$(cat "$TEST_DOCKER_LOG")" = "$(printf 'pull\nup -d')" ]
grep -q 'http://127.0.0.1:8080/health' "$TEST_CURL_LOG"
saved_code=$(cat "$test_tmp/home/bootstrap-access-code")
[ "${#saved_code}" -eq 48 ]
[ "$(cat "$TEST_CODE_LOG")" = "$saved_code" ]
code_output=$(AGBOT_HOME="$test_tmp/home" sh "$test_tmp/bin/agbot" access-code)
[ "$code_output" = "$saved_code" ]
PATH="$test_tmp/bin:$PATH" AGBOT_HOME="$test_tmp/home" sh "$test_tmp/bin/agbot" up
[ "$(cat "$test_tmp/home/bootstrap-access-code")" = "$saved_code" ]
PATH="$test_tmp/bin:$PATH" AGBOT_HOME="$test_tmp/home" AGBOT_BOOTSTRAP_ACCESS_CODE=explicit-private-code \
    sh "$test_tmp/bin/agbot" up
[ "$(cat "$TEST_CODE_LOG")" = explicit-private-code ]
[ "$(cat "$test_tmp/home/bootstrap-access-code")" = "$saved_code" ]
# Both BSD/macOS and GNU/Linux ls display permission bits consistently.
permissions=$(LC_ALL=C ls -l "$test_tmp/home/bootstrap-access-code")
case "$permissions" in -rw-------*) ;; *) printf 'FAIL: code file is not private\n' >&2; exit 1 ;; esac
if PATH="$test_tmp/bin:$PATH" AGBOT_HOME="$test_tmp/home" TEST_PULL_FAILURE=true \
    sh "$test_tmp/bin/agbot" up > "$test_tmp/failure.log" 2>&1; then
    printf 'FAIL: unavailable release was reported as started\n' >&2
    exit 1
fi
grep -q 'Confirm the selected release has been published' "$test_tmp/failure.log"
for maintenance_command in status down logs upgrade; do
    PATH="$test_tmp/bin:$PATH" AGBOT_HOME="$test_tmp/home" \
        sh "$test_tmp/bin/agbot" "$maintenance_command"
    [ "$(cat "$test_tmp/home/bootstrap-access-code")" = "$saved_code" ]
done
if [ -f "$test_root/Start AGBot.command" ]; then
    for launcher in start.sh stop.sh 'Start AGBot.command' 'Stop AGBot.command'; do
        (cd /; PATH="$test_tmp/bin:$PATH" AGBOT_HOME="$test_tmp/home" "$test_root/$launcher")
    done
fi
AGBOT_HOME="$test_tmp/home" sh "$test_tmp/bin/agbot" config set port 9191
config_output=$(AGBOT_HOME="$test_tmp/home" AGBOT_PORT=9292 AGBOT_IMAGE_REPO=example.invalid/custom \
    AGBOT_CHANNEL=edge sh "$test_tmp/bin/agbot" config)
printf '%s\n' "$config_output" | grep -q 'port       = 9292'
printf '%s\n' "$config_output" | grep -q 'image      = example.invalid/custom:edge'
printf 'agbot-cli-test: installed symlinks and startup passed\n'
