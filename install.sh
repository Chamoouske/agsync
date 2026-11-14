#!/bin/sh
set -eu

version=""
while [ $# -gt 0 ]; do
    case "$1" in
        --version)
            [ $# -ge 2 ] || { echo "agsync install: --version precisa de um valor" >&2; exit 2; }
            version="$2"
            shift 2
            ;;
        --version=*)
            version="${1#--version=}"
            shift
            ;;
        *)
            echo "agsync install: opcao desconhecida: $1" >&2
            exit 2
            ;;
    esac
done

case "$(uname -m)" in
    x86_64 | amd64) ;;
    *)
        echo "agsync install: so ha binario para x86_64, esta maquina e $(uname -m)" >&2
        exit 1
        ;;
esac

base="${AGSYNC_INSTALL_BASE_URL:-https://github.com/Chamoouske/agsync/releases}"
base="${base%/}"
asset="agsync-x86_64-unknown-linux-musl"
if [ -n "$version" ]; then
    case "$version" in
        v*) tag="$version" ;;
        *) tag="v$version" ;;
    esac
    url="$base/download/$tag"
else
    url="$base/latest/download"
fi

download() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL -o "$2" "$1"
    else
        wget -q -O "$2" "$1"
    fi
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
download "$url/$asset" "$tmp/$asset"
download "$url/SHA256SUMS" "$tmp/SHA256SUMS"

expected="$(awk -v name="$asset" '$2 == name || $2 == "*" name { print tolower($1) }' "$tmp/SHA256SUMS")"
if [ -z "$expected" ]; then
    echo "agsync install: SHA256SUMS nao tem $asset" >&2
    exit 1
fi
actual="$(sha256sum "$tmp/$asset" | awk '{ print $1 }')"
if [ "$actual" != "$expected" ]; then
    echo "agsync install: SHA256 de $asset nao confere: esperado $expected, veio $actual" >&2
    exit 1
fi

bin="$HOME/.local/bin"
mkdir -p "$bin"
cp "$tmp/$asset" "$bin/agsync.new"
chmod 0755 "$bin/agsync.new"
mv -f "$bin/agsync.new" "$bin/agsync"

case ":$PATH:" in
    *":$bin:"*) ;;
    *)
        line='export PATH="$HOME/.local/bin:$PATH"'
        for rc in "$HOME/.profile" "$HOME/.bashrc" "$HOME/.zshrc"; do
            if [ "$rc" != "$HOME/.profile" ] && [ ! -f "$rc" ]; then
                continue
            fi
            if [ -f "$rc" ] && grep -qxF "$line" "$rc"; then
                continue
            fi
            printf '\n%s\n' "$line" >> "$rc"
            echo "PATH: $bin adicionado em $rc; abra um terminal novo"
        done
        ;;
esac

echo "agsync instalado em $bin/agsync"
echo "proximo passo: agsync init <url> --profile <perfil>"
