# Codigos de saida

| codigo | quando |
|---|---|
| `0` | sucesso, inclusive quando voce responde nao a uma confirmacao |
| `1` | erro (mensagem `erro: ...` na stderr) ou verificacao com problema |
| `2` | argumento invalido ou desconhecido na linha de comando (erro do parser) |

Casos de `1` em que o comando foi ate o fim, mas algo nao ficou certo:

- `apply` e `rollback`: algum build ou item falhou (`erro: N item(ns) falharam; os demais foram aplicados`); os outros itens foram aplicados e estao no state.
- `doctor`: algum problema encontrado.
- `schedule status`: agendamento ausente ou apontando para outro binario.
- `init`: algum passo falhou (os seguintes ainda rodam quando fazem sentido).
- `hook install`: falhou em algum shell.
- `uninstall`: alguma entrada do state nao saiu de um agente; nesse caso o `state.json` e mantido.
- `migrate-legacy`: algum item nao foi removido.

O `check` nao trata remoto inacessivel como erro: registra no log e sai com `0`. Com o lock ocupado por outra execucao, tambem sai com `0`, em silencio.

Os scripts de instalacao saem com `1` em erro de download, de hash ou de arquitetura, e o `install.sh` sai com `2` em opcao invalida.
