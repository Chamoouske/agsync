# Assumir instalacoes manuais

Um MCP, skill ou plugin instalado a mao que tem exatamente o mesmo nome, o mesmo tipo e o mesmo agente de um item do `agsync.yaml` e uma correspondencia. Por padrao o agsync nao mexe nela:

- `status` e `diff` mostram o item como `conflito` e listam todas as correspondencias no fim;
- `check` registra as correspondencias no `pending.json` (`conflicts`) e na notificacao;
- `apply` sem `--adopt` nao mexe nelas.

## Substituir pela definicao do repo

```sh
agsync diff
agsync apply --adopt
```

O `apply --adopt` lista, antes da confirmacao, as instalacoes manuais que serao substituidas. Depois troca cada uma pela definicao do `agsync.yaml` (origem, versao/ref e configuracao), com backup, e passa a gerencia-la no `state.json`.

Se a substituicao de um item falha, o erro e mostrado, o item nao entra no `state.json` e continua como conflito no proximo `apply`.

## O que conta como correspondencia

- Nome diferente, outro tipo (uma skill `x` nao corresponde a um MCP `x`) ou outro agente nunca contam.
- Plugins no Claude Code: um plugin manual com o mesmo nome vindo de outro marketplace tambem conta, se for o unico com esse nome; o `--adopt` instala o do `agsync.yaml` e desinstala o manual.
- Plugins no OpenCode: so conta o mesmo pacote.
- Plugins no Antigravity: conta o plugin instalado com o mesmo nome, de qualquer origem.

## Diferente de desvio manual

Uma entrada que o agsync escreveu e alguem mudou ou apagou a mao e um desvio, nao um conflito. O proximo `apply` sobrescreve o desvio, com backup, sem precisar de `--adopt`. Veja [Planejamento e state](../explicacao/planejamento.md).
