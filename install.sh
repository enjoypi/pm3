#!/bin/sh
# pm3 一行安装器：curl -fsSL https://raw.githubusercontent.com/enjoypi/pm3/main/install.sh | sh
set -eu

REPO="enjoypi/pm3"

die() {
    echo "install.sh: $*" >&2
    exit 1
}

warn() {
    echo "install.sh: 警告: $*" >&2
}

command -v curl >/dev/null 2>&1 || die "需要 curl"
command -v tar >/dev/null 2>&1 || die "需要 tar"

os=$(uname -s)
arch=$(uname -m)
case "$os/$arch" in
    Darwin/arm64) target="aarch64-apple-darwin" ;;
    Linux/x86_64) target="x86_64-unknown-linux-gnu" ;;
    Linux/aarch64 | Linux/arm64) target="aarch64-unknown-linux-gnu" ;;
    *)
        die "没有 $os/$arch 的预编译产物；可改用 cargo install --git https://github.com/$REPO --bin pm3 从源码安装"
        ;;
esac

tag=$(curl -fsSL -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest" | sed 's|.*/||') || tag=""
case "$tag" in
    v*) ;;
    *)
        tag=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p')
        ;;
esac
case "$tag" in
    v*) ;;
    *) die "取不到最新版本号（github 网络不通？）" ;;
esac
echo "install.sh: 安装 pm3 $tag ($target)"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

asset="pm3-$tag-$target.tar.gz"
base="https://github.com/$REPO/releases/download/$tag"
curl -fsSL -o "$tmp/$asset" "$base/$asset" || die "下载 $asset 失败"
curl -fsSL -o "$tmp/$asset.sha256" "$base/$asset.sha256" || die "下载 $asset.sha256 失败"

if command -v sha256sum >/dev/null 2>&1; then
    (cd "$tmp" && sha256sum -c "$asset.sha256") || die "sha256 校验失败，安装中止"
elif command -v shasum >/dev/null 2>&1; then
    (cd "$tmp" && shasum -a 256 -c "$asset.sha256") || die "sha256 校验失败，安装中止"
else
    die "系统既没有 sha256sum 也没有 shasum，无法校验下载完整性"
fi

tar -xzf "$tmp/$asset" -C "$tmp" || die "解包失败"
chmod +x "$tmp/pm3"

[ -x /bin/ps ] || warn "缺 /bin/ps（procps）：daemon 重启后所有服务会被误判探测失败而驱逐重启"
[ -x /bin/kill ] || warn "缺 /bin/kill（procps）：服务停止与身份令牌采集不可用"
if [ "$os" = "Linux" ] && ! command -v bwrap >/dev/null 2>&1; then
    warn "缺 bwrap（bubblewrap）：默认沙箱起不来；安装 bubblewrap 或把 config.yaml 的 sandbox.mode 改为 danger-full-access"
fi

default_cfg="${PM3_HOME:-$HOME/.pm3}/config.yaml"
if [ -f "$default_cfg" ]; then
    "$tmp/pm3" install
else
    "$tmp/pm3" --config "$tmp/config.yaml" install
fi

echo "install.sh: 完成。pm3 已落位（默认 ~/bin/pm3，可用 PM3_INSTALL_PATH 覆盖）并注册开机自启。"
case ":$PATH:" in
    *":$HOME/bin:"*) ;;
    *) warn "~/bin 不在 PATH 里，直接使用请把它加进 shell 配置" ;;
esac
