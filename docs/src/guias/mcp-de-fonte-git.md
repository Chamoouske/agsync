# MCP construído de um repo git

Um MCP stdio pode ser construído a partir do código de um repo git, em vez de um pacote publicado.

```yaml
mcps:
  internal-tool:
    profiles: [work]
    command: node
    args: ["{dir}/dist/index.js"]
    env: {TOKEN: "${INTERNAL_TOKEN}"}
    source:
      git: https://github.com/<org>/internal-tool
      ref: v1.2.0
      requires: [node, npm]
      build: ["npm ci", "npm run build"]
```

O que acontece no `apply`:

1. O agsync resolve a `ref` para um commit e faz um checkout raso em `<dados>/sources/<nome>/<sha>/`.
2. Se falta algum programa de `requires` no PATH, o item é pulado e aparece em `status` e `doctor`.
3. Os comandos de `build` rodam no checkout, em ordem: por `sh -c` no Linux e `cmd /c` no Windows.
4. `{dir}` em `command`, `args` e `env` vira a pasta do checkout.
5. O MCP e escrito nos agentes.

A saída de cada build vai para `<dados>/logs/build-<nome>-<sha>-<ms>.log`. Se um build falha, o erro nomeia o log, o MCP é pulado, os outros itens seguem e o `apply` termina com código `1`.

Regras:

- `git` e `ref` são obrigatórios. O `ref` aceita branch, tag ou SHA completo; SHA abreviado é recusado.
- URL com senha ou token embutido é recusada. Credenciais ficam no credential helper do git.
- `git` ou `ref` que começa com `-` é recusado.
- Uma skill com `git` não pode ter o mesmo nome de um MCP com `source`; use `name` na skill.
- Ref de branch ou tag é seguida: um commit novo nela aparece no `check` como pendência, mesmo sem mudança no repo de config.

## Confirmação

A confirmação do `apply` lista cada build, com repo, commit e comandos, antes de rodar qualquer coisa. Leia antes de confirmar: o build executa código de terceiros na sua máquina. Veja [Segurança](../explicacao/seguranca.md).

Checkouts que nenhum item do state usa mais são removidos depois do `apply`.
