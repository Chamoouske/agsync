# Migrar do agent-config

O sistema antigo (APM, 1MCP e os scripts do agent-config) é substituído assim:

1. Rode `agsync init <url do repo com agsync.yaml> --profile <perfil>`.
2. Rode `agsync migrate-legacy`. Ele lista o que achou, pede confirmação e guarda tudo em `<dados>/backups/legacy-<data>/` antes de remover, item por item:
   - Windows: as tarefas `1mcp-serve` e `agent-config-update` (o `1mcp serve` em execução é parado antes) e a variável de usuário `ONE_MCP_CONFIG`;
   - Linux: `1mcp.service` com o drop-in `1mcp.service.d/`, `agent-config-update.service`/`.timer` (`disable --now`, depois `daemon-reload`) e as linhas `export ONE_MCP_CONFIG=` do `~/.bashrc` e do `~/.zshrc`;
   - o MCP `1mcp` (`command: 1mcp`) em cada agente detectado. É a única entrada fora do `state.json` que o agsync remove, e só neste comando;
   - `~/.apm/`, que é movido inteiro para o backup.
3. O `migrate-legacy` não desinstala programas. Ele imprime os comandos para você rodar: `npm uninstall -g @1mcp/agent` é o do APM, conforme a forma de instalação detectada.
4. Skills instaladas pelo APM ou pelo `npx skills` ficam onde estão. O `migrate-legacy` diz, para cada uma:
   - se o `agsync.yaml` ativo tem o mesmo nome. Nesse caso, `agsync apply --adopt` assume a skill;
   - senão, que ela fica como entrada manual.
5. Rode `agsync apply --adopt`.

Sem terminal, o `migrate-legacy` só roda com `--yes`.
