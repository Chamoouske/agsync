# agsync

O agsync aplica MCP servers, skills e plugins de um repositorio git de configuracao na configuracao de usuario dos agentes de codigo da maquina: Claude Code, Codex, OpenCode, Hermes e Antigravity CLI.

Voce descreve o que quer em um arquivo `agsync.yaml`, na raiz de um repo git. Cada maquina aponta para esse repo e escolhe seus perfis, e o agsync mantem os agentes em dia com o repo.

## Conceitos centrais

- **Repo de config**: um repo git com o `agsync.yaml` na raiz e, opcionalmente, pastas de skills. E a fonte da verdade. Veja [Montar o repo de config](guias/repo-de-config.md).
- **Item**: um MCP server, uma skill ou um plugin declarado no `agsync.yaml`.
- **Agente**: um agente de codigo detectado pelo executavel no PATH (`claude`, `codex`, `opencode`, `hermes`, `agy`). Veja [Agentes](referencia/agentes.md).
- **Perfil**: um rotulo (`work`, `personal`) que restringe itens a algumas maquinas. Veja [Perfis](guias/perfis.md).
- **Maquina**: a configuracao local em `machine.toml`: repo, ref seguida, perfis ativos e intervalo do `check`. Veja [machine.toml](referencia/machine-toml.md).
- **State**: o `state.json`, que registra tudo o que o agsync escreveu em cada agente. O agsync so mexe no que esta nele. Veja [Planejamento e state](explicacao/planejamento.md).
- **Pendencia**: o que o `check` periodico achou de novo no remoto e o `apply` mudaria. Fica em `pending.json`.
- **Import**: outro repo de config de onde vem MCPs, skills ou plugins de um tipo. Veja [Importar de outros repos](guias/imports.md).

## Principios

- So mexe no que ele mesmo escreveu. Entradas manuais com o mesmo nome viram conflito e ficam como estao, a nao ser com `agsync apply --adopt`.
- Escreve pelo CLI do agente quando o CLI sabe fazer a mudanca. Senao, edita o arquivo preservando comentarios.
- Faz backup antes de cada mudanca.
- O `agsync check` periodico so le o remoto e avisa. Nada e aplicado sem `agsync apply`.
- O `agsync hook`, que roda quando o shell abre, nunca acessa a rede.

## Por onde comecar

1. [Instale](primeiros-passos/instalacao.md) o agsync.
2. Siga o [Primeiro uso](primeiros-passos/primeiro-uso.md).
3. Consulte a [Referencia](referencia/cli.md) quando precisar de um detalhe.

Algo deu errado? Veja [Solucao de problemas](solucao-de-problemas.md).

## Licenca

O agsync e distribuido sob a [licenca MIT](https://github.com/Chamoouske/agsync/blob/master/LICENSE).
