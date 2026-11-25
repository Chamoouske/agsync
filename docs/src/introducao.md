# agsync

O agsync aplica MCP servers, skills e plugins de um repositório git de configuração na configuração de usuário dos agentes de código da máquina: Claude Code, Codex, OpenCode, Hermes e Antigravity CLI.

Você descreve o que quer em um arquivo `agsync.yaml`, na raiz de um repo git. Cada máquina aponta para esse repo e escolhe seus perfis, e o agsync mantém os agentes em dia com o repo.

## Conceitos centrais

- **Repo de config**: um repo git com o `agsync.yaml` na raiz e, opcionalmente, pastas de skills. É a fonte da verdade. Veja [Montar o repo de config](guias/repo-de-config.md).
- **Item**: um MCP server, uma skill ou um plugin declarado no `agsync.yaml`.
- **Agente**: um agente de código detectado pelo executável no PATH (`claude`, `codex`, `opencode`, `hermes`, `agy`). Veja [Agentes](referencia/agentes.md).
- **Perfil**: um rótulo (`work`, `personal`) que restringe itens a algumas máquinas. Veja [Perfis](guias/perfis.md).
- **Máquina**: a configuração local em `machine.toml`: repo, ref seguida, perfis ativos e intervalo do `check`. Veja [machine.toml](referencia/machine-toml.md).
- **State**: o `state.json`, que registra tudo o que o agsync escreveu em cada agente. O agsync só mexe no que está nele. Veja [Planejamento e state](explicacao/planejamento.md).
- **Pendência**: o que o `check` periódico achou de novo no remoto e o `apply` mudaria. Fica em `pending.json`.
- **Import**: outro repo de config de onde vêm MCPs, skills ou plugins de um tipo. Veja [Importar de outros repos](guias/imports.md).

## Princípios

- Só mexe no que ele mesmo escreveu. Entradas manuais com o mesmo nome viram conflito e ficam como estão, a não ser com `agsync apply --adopt`.
- Escreve pelo CLI do agente quando o CLI sabe fazer a mudança. Senão, edita o arquivo preservando comentários.
- Faz backup antes de cada mudança.
- O `agsync check` periódico só lê o remoto e avisa. Nada é aplicado sem `agsync apply`.
- O `agsync hook`, que roda quando o shell abre, nunca acessa a rede.

## Por onde começar

1. [Instale](primeiros-passos/instalacao.md) o agsync.
2. Siga o [Primeiro uso](primeiros-passos/primeiro-uso.md).
3. Consulte a [Referência](referencia/cli.md) quando precisar de um detalhe.

Algo deu errado? Veja [Solução de problemas](solucao-de-problemas.md). O que mudou em cada versão está no [Changelog](changelog.md).

## Licença

O agsync é distribuído sob a [licença MIT](https://github.com/Chamoouske/agsync/blob/master/LICENSE).
