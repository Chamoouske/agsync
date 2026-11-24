# Assumir instalações manuais

Um MCP, skill ou plugin instalado à mão que tem exatamente o mesmo nome, o mesmo tipo e o mesmo agente de um item do `agsync.yaml` é uma correspondência. Por padrão o agsync não mexe nela:

- `status` e `diff` mostram o item como `conflito` e listam todas as correspondências no fim;
- `check` registra as correspondências no `pending.json` (`conflicts`) e na notificação;
- `apply` sem `--adopt` não mexe nelas.

## Substituir pela definição do repo

```sh
agsync diff
agsync apply --adopt
```

O `apply --adopt` lista, antes da confirmação, as instalações manuais que serão substituídas. Depois troca cada uma pela definição do `agsync.yaml` (origem, versão/ref e configuração), com backup, e passa a gerenciá-la no `state.json`.

Se a substituição de um item falha, o erro é mostrado, o item não entra no `state.json` e continua como conflito no próximo `apply`.

## O que conta como correspondência

- Nome diferente, outro tipo (uma skill `x` não corresponde a um MCP `x`) ou outro agente nunca contam.
- Plugins no Claude Code: um plugin manual com o mesmo nome vindo de outro marketplace também conta, se for o único com esse nome; o `--adopt` instala o do `agsync.yaml` e desinstala o manual.
- Plugins no OpenCode: só conta o mesmo pacote.
- Plugins no Antigravity: conta o plugin instalado com o mesmo nome, de qualquer origem.

## Diferente de desvio manual

Uma entrada que o agsync escreveu e alguém mudou ou apagou à mão é um desvio, não um conflito. O próximo `apply` sobrescreve o desvio, com backup, sem precisar de `--adopt`. Veja [Planejamento e state](../explicacao/planejamento.md).
