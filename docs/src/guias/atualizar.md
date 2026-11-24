# Atualizar o agsync

O `agsync check` periódico também procura versão nova do agsync, pelas tags `vX.Y.Z` do repo do agsync, lidas com `git ls-remote`. Achando uma, grava `update.json` e notifica uma vez por versão. O `agsync hook` e os outros comandos passam a mostrar:

```text
agsync <nova> disponivel (atual <atual>): rode `agsync update`
```

## Atualizar

```sh
agsync update
```

O `update` baixa o binário da última release, confere o SHA256 com o `SHA256SUMS` e troca o executável. Sem terminal, rode com `--yes`.

- Instalado pelo Scoop: o `update` só diz para rodar `scoop update agsync`, porque o Scoop não atualiza um app em execução.
- No Windows, o binário anterior fica como `agsync.exe.old` até o próximo `update`. Se o novo não executar, o anterior volta; nesse caso o Defender ou o SmartScreen provavelmente bloqueou o binário novo. Veja [Microsoft Defender e SmartScreen](../solucao-de-problemas.md#microsoft-defender-e-smartscreen).
- Sem rede, o `check` só registra a falha no log.

Depois de atualizar, confira se o agendamento ainda aponta para este binário:

```sh
agsync schedule status
```
