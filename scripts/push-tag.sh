#!/usr/bin/env bash
# push-tag.sh - 从 Cargo.toml 读取版本号，创建 tag 并推送到远端
# 用法: ./scripts/push-tag.sh [remote]

set -euo pipefail

REMOTE="${1:-github}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# 从 Cargo.toml 读取版本号
VERSION=$(grep -m1 '^version' "$SCRIPT_DIR/../Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')
if [ -z "$VERSION" ]; then
    echo "错误: 无法从 Cargo.toml 中读取版本号" >&2
    exit 1
fi

TAG="v$VERSION"
echo "版本: $VERSION, Tag: $TAG, Remote: $REMOTE"

# 检查本地是否已有该 tag
if git tag -l "$TAG" | grep -q "$TAG"; then
    echo "本地 tag $TAG 已存在"
else
    echo "创建本地 tag: $TAG"
    git tag "$TAG"
fi

# 检查远端是否已有该 tag
echo "检查远端 $REMOTE 是否存在 tag $TAG ..."
if git ls-remote --tags "$REMOTE" "refs/tags/$TAG" | grep -q "refs/tags/$TAG"; then
    echo "远端 $REMOTE 已存在 tag $TAG，无需推送"
    exit 0
fi

# 推送 tag
echo "推送 tag $TAG 到 $REMOTE ..."
git push "$REMOTE" "$TAG"
echo "✅ 成功推送 tag $TAG 到 $REMOTE"
