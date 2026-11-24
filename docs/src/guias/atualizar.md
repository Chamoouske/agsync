# Atualizar o agsync

O `agsync check` periodico tambem procura versao nova do agsync, pelas tags `vX.Y.Z` do repo do agsync, lidas com `git ls-remote`. Achando uma, grava `update.json` e notifica uma vez por versao. O `agsync hook` e os outros comandos passam a mostrar:

```text
agsync <nova> disponivel (atual <atual>): rode `agsync update`
```

## Atualizar

```sh
agsync update
```

O `update` baixa o binario da ultima release, confere o SHA256 com o `SHA256SUMS` e troca o executavel. Sem terminal, rode com `--yes`.

- Instalado pelo Scoop: o `update` so diz para rodar `scoop update agsync`, porque o Scoop nao atualiza um app em execucao.
- No Windows, o binario anterior fica como `agsync.exe.old` ate o proximo `update`. Se o novo nao executar, o anterior volta; nesse caso o Defender ou o SmartScreen provavelmente bloqueou o binario novo. Veja [Microsoft Defender e SmartScreen](../solucao-de-problemas.md#microsoft-defender-e-smartscreen).
- Sem rede, o `check` so registra a falha no log.

Depois de atualizar, confira se o agendamento ainda aponta para este binario:

```sh
agsync schedule status
```
