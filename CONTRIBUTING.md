# Contribuindo

Obrigado por contribuir com o agsync. Este arquivo é o resumo; os detalhes ficam na seção [Contribuição](https://chamoouske.github.io/agsync/contribuicao/desenvolvimento.html) da documentação (fonte em [`docs/src/contribuicao/`](docs/src/contribuicao/)).

## Compilar, testar e lint

Requer Rust estável com `rustfmt` e `clippy`, e `git` no PATH.

```sh
cargo build
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

São os mesmos passos do CI. Para rodar sem mexer na sua máquina e para os testes dos scripts de packaging, veja [Desenvolvimento](docs/src/contribuicao/desenvolvimento.md).

## Branches e PR

- Mudança normal: crie a branch a partir de `stage` e abra o PR para `stage`.
- Hotfix: crie a branch a partir de `master` e abra o PR para `master`.
- `stage` é promovido para `master` uma vez por dia, e o merge em `master` gera a release.
- Commits seguem [Conventional Commits](https://www.conventionalcommits.org/): o tipo decide a próxima versão.
- O PR só entra com os checks `check (ubuntu-latest)` e `check (windows-latest)` verdes.
- O título do PR e as mensagens dos commits seguem Conventional Commits; o [`CHANGELOG.md`](CHANGELOG.md) é gerado na release a partir dos commits, então não edite à mão.
- Mudou a CLI ou o comportamento? Atualize a documentação em `docs/` (veja [Esta documentação](docs/src/contribuicao/documentacao.md)).

Detalhes do fluxo e da CI em [Branches e CI](docs/src/contribuicao/branches-e-ci.md); releases e CHANGELOG em [Releases](docs/src/contribuicao/releases.md).

## Bugs e ideias

Abra uma [issue](https://github.com/Chamoouske/agsync/issues/new/choose) com o modelo de bug ou de funcionalidade. Antes, veja [Solução de problemas](docs/src/solucao-de-problemas.md).

## Licença

Ao contribuir, você concorda que sua contribuição seja licenciada sob a [licença MIT](LICENSE) do projeto.
