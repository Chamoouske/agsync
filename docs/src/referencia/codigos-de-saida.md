# Códigos de saída

| código | quando |
|---|---|
| `0` | sucesso, inclusive quando você responde não a uma confirmação |
| `1` | erro (mensagem `erro: ...` na stderr) ou verificação com problema |
| `2` | argumento inválido ou desconhecido na linha de comando (erro do parser) |

Casos de `1` em que o comando foi até o fim, mas algo não ficou certo:

- `apply` e `rollback`: algum build ou item falhou (`erro: N item(ns) falharam; os demais foram aplicados`); os outros itens foram aplicados e estão no state.
- `doctor`: algum problema encontrado.
- `schedule status`: agendamento ausente ou apontando para outro binário.
- `init`: algum passo falhou (os seguintes ainda rodam quando fazem sentido).
- `hook install`: falhou em algum shell.
- `uninstall`: alguma entrada do state não saiu de um agente; nesse caso o `state.json` é mantido.
- `migrate-legacy`: algum item não foi removido.

O `check` não trata remoto inacessível como erro: registra no log e sai com `0`. Com o lock ocupado por outra execução, também sai com `0`, em silêncio.

Os scripts de instalação saem com `1` em erro de download, de hash ou de arquitetura, e o `install.sh` sai com `2` em opção inválida.
