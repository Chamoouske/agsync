# Desinstalar

```sh
agsync uninstall
```

O `uninstall`:

1. tira dos agentes tudo o que esta no `state.json` (MCPs, skills e plugins);
2. remove o agendamento do `check`;
3. remove o hook dos perfis de shell;
4. apaga `state.json` e `pending.json`.

Entradas manuais nunca sao tocadas. Os dados dos plugins ficam no disco (`--keep-data` no Claude), e os marketplaces continuam registrados no Claude.

Para apagar tambem as pastas de config e de dados, incluindo `secrets.env`, cache e **todos os backups**:

```sh
agsync uninstall --purge
```

O binario nunca e removido; o comando imprime onde ele esta. Apague-o a mao:

- Windows (script): `%LOCALAPPDATA%\agsync\bin\agsync.exe`, e tire a pasta do PATH do usuario.
- Scoop: `scoop uninstall agsync`.
- Linux: `~/.local/bin/agsync`.

Sem terminal, o `uninstall` so roda com `--yes`.
