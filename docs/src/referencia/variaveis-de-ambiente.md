# Variaveis de ambiente

## Do agsync

| variavel | efeito |
|---|---|
| `AGSYNC_CONFIG_DIR` | troca a pasta de config (`machine.toml`, `secrets.env`) |
| `AGSYNC_DATA_DIR` | troca a pasta de dados (state, cache, backups, logs) |
| `AGSYNC_INSTALL_BASE_URL` | base das releases usada por `install.ps1`, `install.sh` e `agsync update`, no lugar de `https://github.com/Chamoouske/agsync/releases` |
| `AGSYNC_RELEASES_URL` | repo git cujas tags `vX.Y.Z` o `check` le para achar versao nova, no lugar de `https://github.com/Chamoouske/agsync` |
| `AGSYNC_OPENCODE_CONFIG_DIR` | pasta de config do OpenCode usada pelo agsync; tem prioridade sobre `XDG_CONFIG_HOME` |
| `AGSYNC_DOCUMENTS_DIR` | pasta Documentos usada para achar os perfis do PowerShell (`<pasta>/WindowsPowerShell/profile.ps1` e `<pasta>/PowerShell/profile.ps1`), sem perguntar ao PowerShell |

Variaveis com prefixo `AGSYNC_TEST_` existem so para os testes.

## Dos agentes e do sistema

| variavel | efeito |
|---|---|
| `CLAUDE_CONFIG_DIR` | MCPs e skills do Claude Code ficam nessa pasta |
| `CODEX_HOME` | pasta do Codex (padrao `~/.codex`); passada explicitamente a cada chamada do `codex` |
| `HERMES_HOME` | pasta do Hermes (padrao `%LOCALAPPDATA%\hermes` no Windows, `~/.hermes` no Linux); passada a cada chamada do `hermes` |
| `XDG_CONFIG_HOME` | base da config do OpenCode (`$XDG_CONFIG_HOME/opencode`) |
| `OPENCODE_CONFIG_DIR` | ignorada quando herdada; o agsync define a sua em cada chamada do `opencode` |
| `ZDOTDIR` | onde fica o `.zshrc` que recebe o hook |
| `APPDATA`, `LOCALAPPDATA` | base das pastas padrao no Windows |
| `HOME` (Linux), `USERPROFILE` (Windows) | home do usuario |
| `PATH` (e `PATHEXT` no Windows) | deteccao de agentes, `git` e `requires`; congelado no agendamento |
| `DISPLAY`, `WAYLAND_DISPLAY` | notificacao por `notify-send` so com sessao grafica |
| `WSL_DISTRO_NAME` | detecta o WSL |

Os segredos citados como `${VAR}` no `agsync.yaml` tambem sao lidos do ambiente antes do `secrets.env`. Veja [Segredos](../guias/segredos.md).
