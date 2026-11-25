# Arquitetura

O agsync é um único binário Rust (`src/main.rs` com a CLI em clap e `src/lib.rs` com os módulos). Ele não tem servidor nem daemon: cada execução lê o estado local, consulta o remoto pelo `git` do sistema, planeja e, no `apply`, escreve.

## Módulos

```text
CLI (main.rs)
 |
 +-- comandos: init, report (status/diff), apply, rollback, check, hook,
 |             doctor, uninstall, self_update, legacy, cmd (secret)
 |
 +-- entrada:  machine (machine.toml), manifest (agsync.yaml), compose (imports),
 |             git (ls-remote, snapshots), snapshot_cache, import_cache,
 |             resolved (fontes git), source (checkouts e builds), secret, deliver
 |
 +-- decisao:  desired (o que cada agente deve ter), plan (regras de merge),
 |             state (state.json), pending, watch
 |
 +-- saida:    agent/* (um adaptador por agente), agent/link (skills),
 |             agent/backup, plugin, atomic (escrita atomica)
 |
 +-- maquina:  schedule/* (Task Scheduler, systemd, cron), shell/* (hooks),
               notify/* (toast, notify-send), host/* (nativo ou WSL), lock, paths
```

## Interfaces pequenas, uma implementação por caso

Partes que variam por SO ou por agente ficam atrás de uma interface pequena, com uma implementação por caso:

- **Agentes** (`src/agent`): cada adaptador traduz o modelo neutro de MCP (`src/mcp.rs`) para o formato do agente, lê as entradas atuais e escreve pelo CLI do agente ou editando o arquivo.
- **Agendador** (`src/schedule`): Task Scheduler no Windows; timer do systemd de usuário ou crontab no Linux.
- **Shells** (`src/shell`): PowerShell 5.1 e 7, bash, zsh e fish.
- **Notificação** (`src/notify`): toast do Windows via WinRT, `notify-send` no Linux.
- **Host** (`src/host`): Windows/Linux nativo ou Linux dentro do WSL, que ignora os executáveis do Windows.

Os testes trocam essas implementações por versões em arquivo, por variáveis `AGSYNC_TEST_*`, e usam `examples/fake_agent.rs` no lugar dos CLIs reais.

## Fluxo de um apply

1. `Inputs::load` lê `machine.toml`, `state.json` e os segredos.
2. `git ls-remote` resolve a `ref` do repo de config para um commit.
3. O snapshot desse commit vem do cache ou é baixado (`snapshots/`).
4. O `agsync.yaml` é validado; os `imports` dos perfis ativos são resolvidos e juntados (`compose`).
5. As fontes git ativas são resolvidas uma vez (`resolved`).
6. Para cada agente detectado, `desired` calcula o que ele deve ter e `plan` compara com o state e com o que o agente tem hoje.
7. O diff e os builds são mostrados e confirmados.
8. Os builds rodam, cada adaptador faz backup e escreve item por item, e o state é gravado.
9. `pending.json` é limpo, `watch.json` é atualizado e caches sem uso são removidos.

`status` e `diff` fazem os passos 1 a 6 e param. Veja [Planejamento e state](planejamento.md).

## Decisões de design

- **CLI do agente primeiro.** Escrever pelo CLI segue o formato que o próprio agente mantém. Quando o CLI não existe ou não expressa a entrada, a edição do arquivo passa pela árvore de sintaxe (JSONC ou TOML) e preserva comentários e as outras chaves.
- **Só o que escreveu.** O `state.json` separa entradas do agsync das manuais. Nada fora dele é removido, com uma única exceção: o MCP `1mcp` no `migrate-legacy`.
- **Sem aplicar sozinho.** O `check` periódico só avisa; aplicar é sempre um comando explícito (ou `silentUpdate`/`--silent-update` no `init`).
- **Hook sem rede.** O shell abre rápido: o `hook` lê arquivos locais e, no máximo, dispara um `check` em segundo plano.
- **`git` do sistema.** Credenciais, proxies e certificados ficam com a configuração de git da máquina; o agsync nunca pede senha.
- **Commits são imutáveis.** Snapshots, imports e fontes são cacheados por url e commit e nunca ficam velhos; um `ls-remote` basta para saber se algo mudou.
- **Hash sem segredo.** A identidade de cada entrada no state não depende dos valores dos segredos, então trocar um segredo não gera pendência nem vaza no state.
- **Sem admin.** Instalação, agendamento e hooks ficam no escopo do usuário.
