# JustFile for Tuack-NG
set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# 列出可用任务
default:
    @just --list

# 初始化项目所需信息
init:
    # Init git submodule
    git submodule update --init --recursive
    # 提交信息校验钩子（.githooks/commit-msg）
    git config core.hooksPath .githooks
    @if [ -f /etc/NIXOS ]; then \
        just _nixos-init; \
    fi

# 构建项目
build:
    @cargo build

# 以发布模式构建
build-release:
    @cargo build --release --locked

# 运行
[no-cd]
run +args="":
    @cargo run -p tuack-ng -- {{ args }}

[working-directory("assets/templates")]
unwrap:
    @cargo run -p tuack-ng -- develop unwrap

[working-directory("assets/templates")]
wrap:
    @cargo run -p tuack-ng -- develop wrap

# 检查格式
fmt-check:
    cargo fmt --all -- --check

# 检查 lint
lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings

# 运行全部测试
test:
    cargo test --workspace --locked

# 用清单里的最低 Rust 版本检查。调用者负责准备工具链。
msrv:
    cargo check --workspace --all-targets --locked

# 检查依赖的许可证、bans 与 advisory
deny:
    cargo deny check

# 检查拼写
spell:
    typos

# PR 必须通过的全部检查
check: fmt-check lint test

# 完整检查
verify: check deny spell

# 判断变更是否只有文档。输出 code=true 或 code=false。
ci-classify:
    #!/bin/sh
    set -eu
    if [ -z "${BASE_SHA:-}" ]; then
        echo "code=true"
        exit 0
    fi
    if ! paths=$(git diff --name-only "$BASE_SHA...HEAD"); then
        echo "code=true"
        exit 0
    fi
    printf '%s\n' "$paths" | .github/scripts/classify-changes.sh

# 检查 PR 的提交标题是否符合约定。需要 GH_TOKEN、GH_REPO 与 PR_NUMBER。
ci-commit-lint:
    #!/bin/bash
    set -euo pipefail
    pattern='^(feat|fix|perf|refactor|docs|test|ci|build|chore|revert|release)(\([a-z0-9_/-]+\))?!?: .+'
    titles=$(mktemp)
    gh api "repos/$GH_REPO/pulls/$PR_NUMBER/commits?per_page=100" \
        --jq '.[].commit.message | split("\n")[0]' > "$titles"
    status=0
    while IFS= read -r subject; do
        [ -n "$subject" ] || continue
        if ! printf '%s\n' "$subject" | grep -Eq "$pattern"; then
            echo "::error::commit title does not follow the commit convention: $subject"
            status=1
        fi
    done < "$titles"
    exit "$status"

# 检查发布版本号与 Cargo.toml 一致。需要 RELEASE_TAG。
ci-check-version:
    #!/bin/sh
    set -eu
    version=$(awk -F'"' '/^version = / { print $2; exit }' Cargo.toml)
    tag="${RELEASE_TAG#v}"
    if [ "$version" != "$tag" ]; then
        echo "tag $RELEASE_TAG does not match Cargo.toml ($version)" >&2
        exit 1
    fi

# NixOS 底下 build.rs 编译 checkers 炸时使用的东西（笑）
# 但是使用 flake.nix 时不应该使用
[confirm("检测到 NixOS，是否运行 checkers 初始化？[y/N]")]
_nixos-init:
    #!/run/current-system/sw/bin/bash
    # 没错，这是（它宝贝的） NixOS 特色！
    set -e

    CHECKERS_DIR="assets/checkers"

    # 确保目录存在
    if [ ! -d "$CHECKERS_DIR" ]; then
        echo "$CHECKERS_DIR 不存在"
        exit 0
    fi

    # 遍历所有 .cpp 文件
    for cpp_file in "$CHECKERS_DIR"/*.cpp; do
        [ -f "$cpp_file" ] || continue

        # 生成可执行文件名
        exe_name="$(basename "$cpp_file" .cpp)"
        exe_path="$CHECKERS_DIR/$exe_name"

        # 检查是否需要编译
        need_compile=1
        if [ -f "$exe_path" ]; then
            # 比较修改时间
            if [ "$cpp_file" -nt "$exe_path" ]; then
                need_compile=1
            else
                need_compile=0
            fi
        fi

        if [ $need_compile -eq 1 ]; then
            echo "编译: $cpp_file"

            # 编译命令
            g++ -std=c++17 -O2 "$cpp_file" -o "$exe_path"

            if [ $? -eq 0 ]; then
                echo "编译成功: $exe_path"
            else
                echo "编译失败: $cpp_file"
                exit 1
            fi
        else
            echo "跳过: $cpp_file"
        fi
    done
