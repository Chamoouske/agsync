# Esta documentacao

A documentacao fica em `docs/` e e gerada com [mdBook](https://rust-lang.github.io/mdBook/). Ela segue o modelo Diataxis: primeiros passos, guias, referencia e explicacao.

```text
docs/
  book.toml          configuracao do mdBook
  gen-cli-help.sh    gera referencia/cli-help.md a partir do --help
  src/
    SUMMARY.md       indice; toda pagina precisa estar aqui
    ...
```

## Ver localmente

Instale o mdBook (binario das [releases](https://github.com/rust-lang/mdBook/releases) ou `cargo install mdbook`) e rode, na raiz do repo:

```sh
mdbook serve docs --open
```

## Ajuda da CLI gerada

`referencia/cli-help.md` e gerado do `--help` de cada comando. Depois de mudar a CLI em `src/main.rs`, regenere:

```sh
cargo build
docs/gen-cli-help.sh
```

No Windows, passe o binario: `bash docs/gen-cli-help.sh target/debug/agsync.exe`. Um comando novo tambem precisa entrar na lista do script.

## CI e publicacao

O workflow `docs`:

1. compila o agsync, roda `docs/gen-cli-help.sh` e falha se `cli-help.md` mudou;
2. roda `mdbook build docs`;
3. confere os links internos do HTML gerado com o `lychee` (`--offline`);
4. em push em `master`, publica `docs/book` no GitHub Pages, em <https://chamoouske.github.io/agsync/>. Assim o site mostra so o que ja foi lancado; em PRs e em `stage` o workflow so compila e confere.

O workflow `release` ignora mudancas em `docs/` e no proprio workflow `docs`.

## Estilo

- Portugues sem acentos, como o resto do repo.
- Fiel ao codigo: nao documente o que o codigo nao faz. Na duvida, leia o modulo e o doc comment dele.
- Instrucoes de instalacao apontam para os scripts em `master`, que e o que esta publicado.
