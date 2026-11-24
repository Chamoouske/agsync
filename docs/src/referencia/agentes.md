# Agentes

Um agente é detectado pelo executável no PATH. Um agente ausente é pulado e aparece em `status`. Os nomes usados em `agents` no `agsync.yaml` estão na primeira coluna.

| nome | agente | executável |
|---|---|---|
| `claude` | Claude Code | `claude` |
| `codex` | Codex CLI | `codex` |
| `opencode` | OpenCode | `opencode` |
| `hermes` | Hermes Agent | `hermes` |
| `antigravity` | Antigravity CLI (a IDE sozinha não é detectada) | `agy` |

MCPs só valem depois de reiniciar o agente.

## Claude Code

- MCPs: `claude mcp add-json/remove --scope user`. O `.claude.json` é lido, nunca editado pelo agsync.
- Skills: `~/.claude/skills`.
- Com `CLAUDE_CONFIG_DIR`, as duas coisas ficam nessa pasta.
- Plugins: `claude plugin marketplace add` e `plugin install/update/uninstall`.
- Backup: `.claude.json` e `settings.json`.

## Codex

- MCPs: `codex mcp add/remove` quando o CLI expressa a entrada. Senão, o agsync edita `$CODEX_HOME/config.toml` (padrão `~/.codex`), tabela `[mcp_servers.<nome>]`, mantendo comentários.
- Skills: `~/.agents/skills`.
- Plugins: não suportados.

## OpenCode

- Pasta de config: `AGSYNC_OPENCODE_CONFIG_DIR`, senão `$XDG_CONFIG_HOME/opencode`, senão `~/.config/opencode` (o mesmo caminho em todo SO).
- MCPs: `opencode mcp add --global`. O OpenCode não tem `mcp remove`, então a remoção edita o `opencode.json(c)` mantendo comentários.
- Skills: `~/.config/opencode/skills`.
- O OpenCode também lê `~/.claude/skills` e `~/.agents/skills`. O `status` avisa quando uma skill restrita por `agents` fica visível nele por isso.
- Plugins: `opencode plugin add/remove`, gravando o array `plugins`.
- Depois do apply: `opencode reload`.

## Hermes

- Pasta: `HERMES_HOME`, senão `%LOCALAPPDATA%\hermes` no Windows e `~/.hermes` no Linux.
- MCPs: `hermes config set/unset mcp_servers.<nome>` (ficam em `<HERMES_HOME>/config.yaml`).
- Skills: `<HERMES_HOME>/skills`.
- Plugins: não suportados.

## Antigravity CLI

- MCPs: o agsync edita `mcpServers` em `~/.gemini/config/mcp_config.json` mantendo comentários, porque o `agy` não tem comando não interativo para MCP. Remoto vira `serverUrl`. O Antigravity não expande `${VAR}`, então o valor do segredo é gravado no arquivo.
- Skills: `~/.gemini/config/skills`. O `agy` também lê `~/.gemini/antigravity-cli/skills` e `~/.gemini/skills`.
- Plugins: `agy plugin install <pasta>` a partir de um checkout do marketplace na `ref`, e `agy plugin uninstall`; marca em `~/.gemini/config/plugins/<nome>/.agsync-source.json`.
- Depois do apply, no `agy`: `/mcp` e `/skills reload`.

## Skills em todos os agentes

Skills são copiadas para o cache `<dados>/skills` e ligadas na pasta de skills do agente. A ordem de tentativa é symlink, depois junction (Windows), depois cópia. O modo usado fica no `state.json`.
