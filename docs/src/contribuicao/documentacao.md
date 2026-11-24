# Esta documentação

A documentação fica em `docs/` e é gerada com [mdBook](https://rust-lang.github.io/mdBook/). Ela segue o modelo Diátaxis: primeiros passos, guias, referência e explicação.

```text
docs/
  book.toml          configuracao do mdBook
  gen-cli-help.sh    gera referencia/cli-help.md a partir do --help
  src/
    SUMMARY.md       indice; toda pagina precisa estar aqui
    ...
```

## Ver localmente

Instale o mdBook (binário das [releases](https://github.com/rust-lang/mdBook/releases) ou `cargo install mdbook`) e rode, na raiz do repo:

```sh
mdbook serve docs --open
```

## Ajuda da CLI gerada

`referencia/cli-help.md` é gerado do `--help` de cada comando. Depois de mudar a CLI em `src/main.rs`, regenere:

```sh
cargo build
docs/gen-cli-help.sh
```

No Windows, passe o binário: `bash docs/gen-cli-help.sh target/debug/agsync.exe`. Um comando novo também precisa entrar na lista do script.

## CI e publicação

O workflow `docs`:

1. compila o agsync, roda `docs/gen-cli-help.sh` e falha se `cli-help.md` mudou;
2. roda `mdbook build docs`;
3. confere os links internos do HTML gerado com o `lychee` (`--offline`);
4. em push em `master`, publica `docs/book` no GitHub Pages, em <https://chamoouske.github.io/agsync/>. Assim o site mostra só o que já foi lançado; em PRs e em `stage` o workflow só compila e confere.

O workflow `release` ignora mudanças em `docs/` e no próprio workflow `docs`.

## Estilo

- Português sem acentos, como o resto do repo.
- Fiel ao código: não documente o que o código não faz. Na dúvida, leia o módulo e o doc comment dele.
- Instruções de instalação apontam para os scripts em `master`, que é o que está publicado.
