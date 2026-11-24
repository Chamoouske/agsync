# Migrar do agent-config

O sistema antigo (APM, 1MCP e os scripts do agent-config) e substituido assim:

1. Rode `agsync init <url do repo com agsync.yaml> --profile <perfil>`.
2. Rode `agsync migrate-legacy`. Ele lista o que achou, pede confirmacao e guarda tudo em `<dados>/backups/legacy-<data>/` antes de remover, item por item:
   - Windows: as tarefas `1mcp-serve` e `agent-config-update` (o `1mcp serve` em execucao e parado antes) e a variavel de usuario `ONE_MCP_CONFIG`;
   - Linux: `1mcp.service` com o drop-in `1mcp.service.d/`, `agent-config-update.service`/`.timer` (`disable --now`, depois `daemon-reload`) e as linhas `export ONE_MCP_CONFIG=` do `~/.bashrc` e do `~/.zshrc`;
   - o MCP `1mcp` (`command: 1mcp`) em cada agente detectado. E a unica entrada fora do `state.json` que o agsync remove, e so neste comando;
   - `~/.apm/`, que e movido inteiro para o backup.
3. O `migrate-legacy` nao desinstala programas. Ele imprime os comandos para voce rodar: `npm uninstall -g @1mcp/agent` e o do APM, conforme a forma de instalacao detectada.
4. Skills instaladas pelo APM ou pelo `npx skills` ficam onde estao. O `migrate-legacy` diz, para cada uma:
   - se o `agsync.yaml` ativo tem o mesmo nome. Nesse caso, `agsync apply --adopt` assume a skill;
   - senao, que ela fica como entrada manual.
5. Rode `agsync apply --adopt`.

Sem terminal, o `migrate-legacy` so roda com `--yes`.
