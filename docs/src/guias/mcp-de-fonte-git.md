# MCP construido de um repo git

Um MCP stdio pode ser construido a partir do codigo de um repo git, em vez de um pacote publicado.

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
2. Se falta algum programa de `requires` no PATH, o item e pulado e aparece em `status` e `doctor`.
3. Os comandos de `build` rodam no checkout, em ordem: por `sh -c` no Linux e `cmd /c` no Windows.
4. `{dir}` em `command`, `args` e `env` vira a pasta do checkout.
5. O MCP e escrito nos agentes.

A saida de cada build vai para `<dados>/logs/build-<nome>-<sha>-<ms>.log`. Se um build falha, o erro nomeia o log, o MCP e pulado, os outros itens seguem e o `apply` termina com codigo `1`.

Regras:

- `git` e `ref` sao obrigatorios. O `ref` aceita branch, tag ou SHA completo; SHA abreviado e recusado.
- URL com senha ou token embutido e recusada. Credenciais ficam no credential helper do git.
- `git` ou `ref` que comeca com `-` e recusado.
- Uma skill com `git` nao pode ter o mesmo nome de um MCP com `source`; use `name` na skill.
- Ref de branch ou tag e seguida: um commit novo nela aparece no `check` como pendencia, mesmo sem mudanca no repo de config.

## Confirmacao

A confirmacao do `apply` lista cada build, com repo, commit e comandos, antes de rodar qualquer coisa. Leia antes de confirmar: o build executa codigo de terceiros na sua maquina. Veja [Seguranca](../explicacao/seguranca.md).

Checkouts que nenhum item do state usa mais sao removidos depois do `apply`.
