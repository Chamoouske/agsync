# Contribuindo

Obrigado por contribuir com o agsync. Este arquivo e o resumo; os detalhes ficam na secao [Contribuicao](https://chamoouske.github.io/agsync/contribuicao/desenvolvimento.html) da documentacao (fonte em [`docs/src/contribuicao/`](docs/src/contribuicao/)).

## Compilar, testar e lint

Requer Rust estavel com `rustfmt` e `clippy`, e `git` no PATH.

```sh
cargo build
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --no-fail-fast
```

Sao os mesmos passos do CI. Para rodar sem mexer na sua maquina e para os testes dos scripts de packaging, veja [Desenvolvimento](docs/src/contribuicao/desenvolvimento.md).

## Branches e PR

- Mudanca normal: crie a branch a partir de `stage` e abra o PR para `stage`.
- Hotfix: crie a branch a partir de `master` e abra o PR para `master`.
- `stage` e promovido para `master` uma vez por dia, e o merge em `master` gera a release.
- Commits seguem [Conventional Commits](https://www.conventionalcommits.org/): o tipo decide a proxima versao.
- O PR so entra com os checks `check (ubuntu-latest)` e `check (windows-latest)` verdes.
- O titulo do PR e as mensagens dos commits seguem Conventional Commits; o [`CHANGELOG.md`](CHANGELOG.md) e gerado na release a partir dos commits, entao nao edite a mao.
- Mudou a CLI ou o comportamento? Atualize a documentacao em `docs/` (veja [Esta documentacao](docs/src/contribuicao/documentacao.md)).

Detalhes do fluxo e da CI em [Branches e CI](docs/src/contribuicao/branches-e-ci.md); releases e CHANGELOG em [Releases](docs/src/contribuicao/releases.md).

## Bugs e ideias

Abra uma [issue](https://github.com/Chamoouske/agsync/issues/new/choose) com o modelo de bug ou de funcionalidade. Antes, veja [Solucao de problemas](docs/src/solucao-de-problemas.md).

## Licenca

Ao contribuir, voce concorda que sua contribuicao seja licenciada sob a [licenca MIT](LICENSE) do projeto.
