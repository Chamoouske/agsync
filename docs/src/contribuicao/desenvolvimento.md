# Desenvolvimento

## Requisitos

- Rust estavel (edition 2024), com `rustfmt` e `clippy`.
- `git` no PATH.
- Windows ou Linux. No Windows, o `build.rs` embute no `agsync.exe` os metadados de versao e o manifest de `assets/agsync.manifest`.

## Compilar

```sh
cargo build
cargo build --release
```

O binario de desenvolvimento fica em `target/debug/agsync` (`agsync.exe` no Windows). O release usa LTO thin e uma unidade de codegen.

A versao em `Cargo.toml` fica em `0.1.0` no repo: o workflow de release grava a versao calculada antes de compilar. Por isso `agsync --version` de um build local mostra `0.1.0`.

## Rodar sem mexer na sua maquina

Aponte as pastas do agsync para um lugar temporario:

```sh
export AGSYNC_CONFIG_DIR=/tmp/agsync-dev/config
export AGSYNC_DATA_DIR=/tmp/agsync-dev/data
cargo run -- doctor
```

Os agentes reais da maquina ainda sao detectados pelo PATH; `status` e `diff` nao escrevem nada.

## Testar

Os mesmos tres passos do CI:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --no-fail-fast
```

- Testes de unidade ficam nos modulos (`#[cfg(test)]`); testes de integracao, em `tests/`, com utilitarios em `tests/common/`.
- `examples/fake_agent.rs` faz o papel dos CLIs `codex`, `hermes` e `agy`: os testes copiam o binario com esses nomes para uma pasta temporaria no inicio do PATH, e cada chamada e registrada em `$FAKE_AGENT_LOG`.
- Agendador, notificacoes, shells e o sistema legado sao isolados por variaveis `AGSYNC_TEST_*`, entao os testes nao criam tarefas nem tocam perfis reais.
- Alguns testes so rodam em um SO (por exemplo `tests/linux_*.rs`, `tests/wsl.rs`, `tests/windows_cmd.rs`); o CI roda em Windows e Linux.

Os scripts de packaging tem testes proprios, em bash:

```sh
scripts/test-manifests.sh
scripts/test-merge-pr.sh
scripts/test-next-version.sh
scripts/test-check-conventional.sh
```

## Convencoes

- Mensagens para o usuario em portugues, sem acentos; o texto de `--help` da CLI e os doc comments em ingles.
- Documentacao de codigo so com doc comments (`//!` e `///`).
- Commits seguem Conventional Commits: o tipo decide a proxima versao (veja [Releases](releases.md)).
- O `CHANGELOG.md` e gerado na release a partir dos commits; nao edite a mao. A descricao do commit vira a linha do changelog (veja [Changelog](releases.md#changelog)).

O [`CONTRIBUTING.md`](https://github.com/Chamoouske/agsync/blob/master/CONTRIBUTING.md) na raiz do repo resume esta secao para quem chega pelo GitHub.
