# Arquivos e pastas

## Pastas do agsync

| | Windows | Linux | variavel |
|---|---|---|---|
| config | `%APPDATA%\agsync` | `~/.config/agsync` | `AGSYNC_CONFIG_DIR` |
| dados | `%LOCALAPPDATA%\agsync` | `~/.local/share/agsync` | `AGSYNC_DATA_DIR` |
| binario (scripts) | `%LOCALAPPDATA%\agsync\bin\agsync.exe` | `~/.local/bin/agsync` | |

## Na pasta de config

| arquivo | conteudo |
|---|---|
| `machine.toml` | repo, ref, perfis e intervalo do `check`; veja [machine.toml](machine-toml.md) |
| `secrets.env` | valores de `${VAR}`, uma linha `NOME=valor` por segredo; acessivel so pelo usuario |

## Na pasta de dados

| caminho | conteudo |
|---|---|
| `state.json` | o que o agsync escreveu em cada agente (MCPs, skills, plugins, com hash e, para fontes git, commit), mais o commit aplicado e o anterior |
| `pending.json` | o que o ultimo `check` achou que o `apply` mudaria, ou que o `agsync.yaml` remoto e invalido; inclui as instalacoes manuais em conflito |
| `update.json` | versao nova do agsync achada pelo `check` |
| `watch.json` | o ultimo commit planejado e as fontes moveis que ele usou; deixa o `check` parar depois do `ls-remote` quando nada mudou |
| `last-check` | o mtime marca o ultimo `check` que alcancou o remoto |
| `hook-started-check` | o mtime marca o ultimo `check` disparado pelo `hook` |
| `agsync.lock` | so uma execucao por vez escreve state e pendencias |
| `logs/check.log` | uma linha por evento do `check`; renomeado para `check.log.1` acima de 1 MB |
| `logs/build-<nome>-<sha>-<ms>.log` | saida de cada build de MCP com `source` |
| `backups/` | copias com timestamp dos arquivos de config dos agentes e dos perfis de shell, feitas antes de cada mudanca; `legacy-<data>/` do `migrate-legacy`; acessivel so pelo usuario |
| `skills/` | cache das pastas de skill, por conteudo; os agentes recebem links para ca |
| `sources/<nome>/<sha>/` | checkouts das fontes git dos MCPs com `source` |
| `snapshots/` | arquivos do repo de config (sem `.git`) por url e commit |
| `imports/` | `agsync.yaml` validados dos repos importados, por url e commit |
| `agsync-check.sh` | so no Linux com cron: script que o crontab roda, com o PATH congelado |

Entradas de `snapshots/` e `imports/` sem uso ha mais de 30 dias sao removidas. Entradas de `skills/` e `sources/` que nenhum item do state usa sao removidas depois do `apply`.

O `uninstall` apaga `state.json` e `pending.json`; com `--purge`, apaga as duas pastas inteiras.

## Nos agentes

Os arquivos de cada agente que o agsync le ou escreve estao em [Agentes](agentes.md).

## Fora das pastas

| caminho | quem escreve |
|---|---|
| perfis de shell (`~/.bashrc`, `~/.zshrc`, `~/.config/fish/conf.d/agsync.fish`, perfis do PowerShell) | `hook install`, bloco entre `# >>> agsync hook >>>` e `# <<< agsync hook <<<` |
| tarefa `agsync-check` (Windows), `agsync-check.service`/`.timer` no diretorio de usuario do systemd, ou linha do crontab (Linux) | `schedule install` |
| PATH do usuario, `~/.profile`, `~/.bashrc`, `~/.zshrc` | scripts de instalacao |
| `agsync.exe.old` ao lado do binario (Windows) | `update` |
