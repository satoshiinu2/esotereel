#!/usr/bin/env bash
# Esotereel core サーバーのスモークテスト。
#
# core サーバーバイナリを起動し、smoke_client (core/examples/smoke_client.rs)
# で NewProject / FetchClipsInRange / Command(AddLayer) を実行して、
# 各レスポンスが制限時間 (10秒/ステップ) 内に返ることを確認する。
# サーバーがフリーズ/デッドロックしていればタイムアウトで失敗する。
#
# 使い方: scripts/smoke-test.sh   (cargo が必要)
set -euo pipefail

cd "$(dirname "$0")/.."

export LIBCLANG_PATH="${LIBCLANG_PATH:-/usr/lib/llvm-18/lib}"

echo "== building core server and smoke client =="
cargo build -p esotereel_core --bin esotereel_core --example smoke_client

WORKDIR="$(mktemp -d)"
SERVER_LOG="$WORKDIR/server.log"
echo "== starting core server (log: $SERVER_LOG) =="
ESOTEREEL_PLUGIN_DIR="$PWD/plugins/std" ESOTEREEL_WORKING_DIR="$WORKDIR" \
    ./target/debug/esotereel_core >"$SERVER_LOG" 2>&1 &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true; wait "$SERVER_PID" 2>/dev/null || true' EXIT

echo "== running smoke client (timeout 60s) =="
timeout 60 ./target/debug/examples/smoke_client

echo "== checking server log for panics =="
if grep -E "panicked|deadlock" "$SERVER_LOG"; then
    echo "SMOKE TEST FAILED: panic/deadlock symptom in server log" >&2
    exit 1
fi

echo "SMOKE TEST PASSED"
