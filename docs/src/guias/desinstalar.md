# Desinstalar

```sh
agsync uninstall
```

O `uninstall`:

1. tira dos agentes tudo o que está no `state.json` (MCPs, skills e plugins);
2. remove o agendamento do `check`;
3. remove o hook dos perfis de shell;
4. apaga `state.json` e `pending.json`.

Entradas manuais nunca são tocadas. Os dados dos plugins ficam no disco (`--keep-data` no Claude), e os marketplaces continuam registrados no Claude.

Para apagar também as pastas de config e de dados, incluindo `secrets.env`, cache e **todos os backups**:

```sh
agsync uninstall --purge
```

O binário nunca é removido; o comando imprime onde ele está. Apague-o à mão:

- Windows (script): `%LOCALAPPDATA%\agsync\bin\agsync.exe`, e tire a pasta do PATH do usuário.
- Scoop: `scoop uninstall agsync`.
- Linux: `~/.local/bin/agsync`.

Sem terminal, o `uninstall` só roda com `--yes`.
