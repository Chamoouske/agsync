# Agentes

Um agente e detectado pelo executavel no PATH. Um agente ausente e pulado e aparece em `status`. Os nomes usados em `agents` no `agsync.yaml` estao na primeira coluna.

| nome | agente | executavel |
|---|---|---|
| `claude` | Claude Code | `claude` |
| `codex` | Codex CLI | `codex` |
| `opencode` | OpenCode | `opencode` |
| `hermes` | Hermes Agent | `hermes` |
| `antigravity` | Antigravity CLI (a IDE sozinha nao e detectada) | `agy` |

MCPs so valem depois de reiniciar o agente.

## Claude Code

- MCPs: `claude mcp add-json/remove --scope user`. O `.claude.json` e lido, nunca editado pelo agsync.
- Skills: `~/.claude/skills`.
- Com `CLAUDE_CONFIG_DIR`, as duas coisas ficam nessa pasta.
- Plugins: `claude plugin marketplace add` e `plugin install/update/uninstall`.
- Backup: `.claude.json` e `settings.json`.

## Codex

- MCPs: `codex mcp add/remove` quando o CLI expressa a entrada. Senao, o agsync edita `$CODEX_HOME/config.toml` (padrao `~/.codex`), tabela `[mcp_servers.<nome>]`, mantendo comentarios.
- Skills: `~/.agents/skills`.
- Plugins: nao suportados.

## OpenCode

- Pasta de config: `AGSYNC_OPENCODE_CONFIG_DIR`, senao `$XDG_CONFIG_HOME/opencode`, senao `~/.config/opencode` (o mesmo caminho em todo SO).
- MCPs: `opencode mcp add --global`. O OpenCode nao tem `mcp remove`, entao a remocao edita o `opencode.json(c)` mantendo comentarios.
- Skills: `~/.config/opencode/skills`.
- O OpenCode tambem le `~/.claude/skills` e `~/.agents/skills`. O `status` avisa quando uma skill restrita por `agents` fica visivel nele por isso.
- Plugins: `opencode plugin add/remove`, gravando o array `plugins`.
- Depois do apply: `opencode reload`.

## Hermes

- Pasta: `HERMES_HOME`, senao `%LOCALAPPDATA%\hermes` no Windows e `~/.hermes` no Linux.
- MCPs: `hermes config set/unset mcp_servers.<nome>` (ficam em `<HERMES_HOME>/config.yaml`).
- Skills: `<HERMES_HOME>/skills`.
- Plugins: nao suportados.

## Antigravity CLI

- MCPs: o agsync edita `mcpServers` em `~/.gemini/config/mcp_config.json` mantendo comentarios, porque o `agy` nao tem comando nao interativo para MCP. Remoto vira `serverUrl`. O Antigravity nao expande `${VAR}`, entao o valor do segredo e gravado no arquivo.
- Skills: `~/.gemini/config/skills`. O `agy` tambem le `~/.gemini/antigravity-cli/skills` e `~/.gemini/skills`.
- Plugins: `agy plugin install <pasta>` a partir de um checkout do marketplace na `ref`, e `agy plugin uninstall`; marca em `~/.gemini/config/plugins/<nome>/.agsync-source.json`.
- Depois do apply, no `agy`: `/mcp` e `/skills reload`.

## Skills em todos os agentes

Skills sao copiadas para o cache `<dados>/skills` e ligadas na pasta de skills do agente. A ordem de tentativa e symlink, depois junction (Windows), depois copia. O modo usado fica no `state.json`.
