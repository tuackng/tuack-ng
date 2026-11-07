#!/bin/sh
# 把变更列表分成 "代码" 与 "文档"，供 CI 决定是否跳过构建与测试。
#
# 从 stdin 读取路径（每行一个），输出 `code=true` 或 `code=false`。
# 文档是封闭列表，包括 Markdown、doc/ 下的文档与图片、许可证、贡献者名单和
# issue/PR 模板。其他变更一律按代码处理。新文件类型默认运行完整 CI。
# doc/debian/ 是 deb 打包的 maintainer scripts，影响发布产物，因此按代码处理。
set -eu

code=false
while IFS= read -r path; do
    [ -n "$path" ] || continue
    case $path in
        *.md | doc/assets/* | doc/changelog/* | LICENSE* | .all-contributorsrc | \
            .github/ISSUE_TEMPLATE/* | .github/pull_request_template.md) ;;
        *) code=true ;;
    esac
done
printf 'code=%s\n' "$code"
