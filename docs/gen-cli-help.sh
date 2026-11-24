#!/usr/bin/env bash
set -euo pipefail

bin=${1:-target/debug/agsync}
out=${2:-docs/src/referencia/cli-help.md}

commands=(
  ""
  "init"
  "status"
  "diff"
  "apply"
  "rollback"
  "check"
  "hook"
  "hook install"
  "hook remove"
  "doctor"
  "schedule"
  "schedule install"
  "schedule remove"
  "schedule status"
  "migrate-legacy"
  "uninstall"
  "update"
  "secret"
  "secret set"
  "secret list"
)

{
  echo "# Ajuda da CLI (gerada)"
  echo
  echo "Saida de \`agsync <comando> --help\`, gerada por \`docs/gen-cli-help.sh\`. Nao edite a mao: rode o script depois de mudar a CLI. O CI falha se este arquivo estiver desatualizado."
  echo
  echo "O texto de ajuda da CLI e em ingles; a explicacao de cada comando esta em [Comandos](cli.md)."
  for cmd in "${commands[@]}"; do
    echo
    echo "## \`agsync${cmd:+ $cmd}\`"
    echo
    echo '```text'
    read -ra args <<< "$cmd"
    "$bin" "${args[@]}" --help | sed 's/agsync\.exe/agsync/g' | tr -d '\r'
    echo '```'
  done
} > "$out"
