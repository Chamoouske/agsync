# Arquivos e pastas

## Pastas do agsync

| | Windows | Linux | variável |
|---|---|---|---|
| config | `%APPDATA%\agsync` | `~/.config/agsync` | `AGSYNC_CONFIG_DIR` |
| dados | `%LOCALAPPDATA%\agsync` | `~/.local/share/agsync` | `AGSYNC_DATA_DIR` |
| binário (scripts) | `%LOCALAPPDATA%\agsync\bin\agsync.exe` | `~/.local/bin/agsync` | |

## Na pasta de config

| arquivo | conteúdo |
|---|---|
| `machine.toml` | repo, ref, perfis e intervalo do `check`; veja [machine.toml](machine-toml.md) |
| `secrets.env` | valores de `${VAR}`, uma linha `NOME=valor` por segredo; acessível só pelo usuário |

## Na pasta de dados

| caminho | conteúdo |
|---|---|
| `state.json` | o que o agsync escreveu em cada agente (MCPs, skills, plugins, com hash e, para fontes git, commit), mais o commit aplicado e o anterior |
| `pending.json` | o que o último `check` achou que o `apply` mudaria, ou que o `agsync.yaml` remoto é inválido; inclui as instalações manuais em conflito |
| `update.json` | versão nova do agsync achada pelo `check` |
| `watch.json` | o último commit planejado e as fontes móveis que ele usou; deixa o `check` parar depois do `ls-remote` quando nada mudou |
| `last-check` | o mtime marca o último `check` que alcançou o remoto |
| `hook-started-check` | o mtime marca o último `check` disparado pelo `hook` |
| `agsync.lock` | só uma execução por vez escreve state e pendências |
| `logs/check.log` | uma linha por evento do `check`; renomeado para `check.log.1` acima de 1 MB |
| `logs/build-<nome>-<sha>-<ms>.log` | saída de cada build de MCP com `source` |
| `backups/` | cópias com timestamp dos arquivos de config dos agentes e dos perfis de shell, feitas antes de cada mudança; `legacy-<data>/` do `migrate-legacy`; acessível só pelo usuário |
| `skills/` | cache das pastas de skill, por conteúdo; os agentes recebem links para cá |
| `sources/<nome>/<sha>/` | checkouts das fontes git dos MCPs com `source` |
| `snapshots/` | arquivos do repo de config (sem `.git`) por url e commit |
| `imports/` | `agsync.yaml` validados dos repos importados, por url e commit |
| `agsync-check.sh` | só no Linux com cron: script que o crontab roda, com o PATH congelado |

Entradas de `snapshots/` e `imports/` sem uso há mais de 30 dias são removidas. Entradas de `skills/` e `sources/` que nenhum item do state usa são removidas depois do `apply`.

O `uninstall` apaga `state.json` e `pending.json`; com `--purge`, apaga as duas pastas inteiras.

## Nos agentes

Os arquivos de cada agente que o agsync lê ou escreve estão em [Agentes](agentes.md).

## Fora das pastas

| caminho | quem escreve |
|---|---|
| perfis de shell (`~/.bashrc`, `~/.zshrc`, `~/.config/fish/conf.d/agsync.fish`, perfis do PowerShell) | `hook install`, bloco entre `# >>> agsync hook >>>` e `# <<< agsync hook <<<` |
| tarefa `agsync-check` (Windows), `agsync-check.service`/`.timer` no diretório de usuário do systemd, ou linha do crontab (Linux) | `schedule install` |
| PATH do usuário, `~/.profile`, `~/.bashrc`, `~/.zshrc` | scripts de instalação |
| `agsync.exe.old` ao lado do binário (Windows) | `update` |
