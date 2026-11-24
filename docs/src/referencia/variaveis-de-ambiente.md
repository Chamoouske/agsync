# Variáveis de ambiente

## Do agsync

| variável | efeito |
|---|---|
| `AGSYNC_CONFIG_DIR` | troca a pasta de config (`machine.toml`, `secrets.env`) |
| `AGSYNC_DATA_DIR` | troca a pasta de dados (state, cache, backups, logs) |
| `AGSYNC_INSTALL_BASE_URL` | base das releases usada por `install.ps1`, `install.sh` e `agsync update`, no lugar de `https://github.com/Chamoouske/agsync/releases` |
| `AGSYNC_RELEASES_URL` | repo git cujas tags `vX.Y.Z` o `check` lê para achar versão nova, no lugar de `https://github.com/Chamoouske/agsync` |
| `AGSYNC_OPENCODE_CONFIG_DIR` | pasta de config do OpenCode usada pelo agsync; tem prioridade sobre `XDG_CONFIG_HOME` |
| `AGSYNC_DOCUMENTS_DIR` | pasta Documentos usada para achar os perfis do PowerShell (`<pasta>/WindowsPowerShell/profile.ps1` e `<pasta>/PowerShell/profile.ps1`), sem perguntar ao PowerShell |

Variáveis com prefixo `AGSYNC_TEST_` existem só para os testes.

## Dos agentes e do sistema

| variável | efeito |
|---|---|
| `CLAUDE_CONFIG_DIR` | MCPs e skills do Claude Code ficam nessa pasta |
| `CODEX_HOME` | pasta do Codex (padrão `~/.codex`); passada explicitamente a cada chamada do `codex` |
| `HERMES_HOME` | pasta do Hermes (padrão `%LOCALAPPDATA%\hermes` no Windows, `~/.hermes` no Linux); passada a cada chamada do `hermes` |
| `XDG_CONFIG_HOME` | base da config do OpenCode (`$XDG_CONFIG_HOME/opencode`) |
| `OPENCODE_CONFIG_DIR` | ignorada quando herdada; o agsync define a sua em cada chamada do `opencode` |
| `ZDOTDIR` | onde fica o `.zshrc` que recebe o hook |
| `APPDATA`, `LOCALAPPDATA` | base das pastas padrão no Windows |
| `HOME` (Linux), `USERPROFILE` (Windows) | home do usuário |
| `PATH` (e `PATHEXT` no Windows) | detecção de agentes, `git` e `requires`; congelado no agendamento |
| `DISPLAY`, `WAYLAND_DISPLAY` | notificação por `notify-send` só com sessão gráfica |
| `WSL_DISTRO_NAME` | detecta o WSL |

Os segredos citados como `${VAR}` no `agsync.yaml` também são lidos do ambiente antes do `secrets.env`. Veja [Segredos](../guias/segredos.md).
