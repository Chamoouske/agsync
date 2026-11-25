# Desenvolvimento

## Requisitos

- Rust estável (edition 2024), com `rustfmt` e `clippy`.
- `git` no PATH.
- Windows ou Linux. No Windows, o `build.rs` embute no `agsync.exe` os metadados de versão e o manifest de `assets/agsync.manifest`.

## Compilar

```sh
cargo build
cargo build --release
```

O binário de desenvolvimento fica em `target/debug/agsync` (`agsync.exe` no Windows). O release usa LTO thin e uma unidade de codegen.

A versão em `Cargo.toml` fica em `0.1.0` no repo: o workflow de release grava a versão calculada antes de compilar. Por isso `agsync --version` de um build local mostra `0.1.0`.

## Rodar sem mexer na sua máquina

Aponte as pastas do agsync para um lugar temporário:

```sh
export AGSYNC_CONFIG_DIR=/tmp/agsync-dev/config
export AGSYNC_DATA_DIR=/tmp/agsync-dev/data
cargo run -- doctor
```

Os agentes reais da máquina ainda são detectados pelo PATH; `status` e `diff` não escrevem nada.

## Testar

Os mesmos três passos do CI:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --no-fail-fast
```

- Testes de unidade ficam nos módulos (`#[cfg(test)]`); testes de integração, em `tests/`, com utilitários em `tests/common/`.
- `examples/fake_agent.rs` faz o papel dos CLIs `codex`, `hermes` e `agy`: os testes copiam o binário com esses nomes para uma pasta temporária no início do PATH, e cada chamada é registrada em `$FAKE_AGENT_LOG`.
- Agendador, notificações, shells e o sistema legado são isolados por variáveis `AGSYNC_TEST_*`, então os testes não criam tarefas nem tocam perfis reais.
- Alguns testes só rodam em um SO (por exemplo `tests/linux_*.rs`, `tests/wsl.rs`, `tests/windows_cmd.rs`); o CI roda em Windows e Linux.

Os scripts de packaging têm testes próprios, em bash:

```sh
scripts/test-manifests.sh
scripts/test-merge-pr.sh
scripts/test-next-version.sh
scripts/test-check-conventional.sh
```

## Convenções

- Mensagens para o usuário em português, sem acentos; o texto de `--help` da CLI e os doc comments em inglês.
- Documentação de código só com doc comments (`//!` e `///`).
- Commits seguem Conventional Commits: o tipo decide a próxima versão (veja [Releases](releases.md)).
- O `CHANGELOG.md` é gerado na release a partir dos commits; não edite à mão. A descrição do commit vira a linha do changelog (veja [Changelog](releases.md#changelog)).

O [`CONTRIBUTING.md`](https://github.com/Chamoouske/agsync/blob/master/CONTRIBUTING.md) na raiz do repo resume esta seção para quem chega pelo GitHub.
