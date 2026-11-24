# Voltar para o commit anterior

Se o ultimo `apply` trouxe algo quebrado, volte para o commit aplicado antes dele:

```sh
agsync rollback
```

- O `rollback` planeja o commit anterior pelo mesmo fluxo de plano e confirmacao do `apply`. Backups nunca sao copiados de volta sobre as configs dos agentes.
- Precisa de dois applies de commits diferentes: o `state.json` guarda so o commit atual e o anterior. Sem anterior, o comando falha.
- Sem terminal, rode com `--yes`.

Depois do `rollback`, o `check` volta a acusar o commit remoto como pendente. Isso e esperado. Corrija o repo de config e rode `agsync apply` para seguir em frente.

## Restaurar um arquivo do backup

Se voce precisa de uma copia exata de um arquivo de config de antes de uma mudanca, os backups estao em `<dados>/backups/` (veja [Arquivos e pastas](../referencia/arquivos.md)). A restauracao e manual.
