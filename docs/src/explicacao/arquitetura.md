# Arquitetura

O agsync e um unico binario Rust (`src/main.rs` com a CLI em clap e `src/lib.rs` com os modulos). Ele nao tem servidor nem daemon: cada execucao le o estado local, consulta o remoto pelo `git` do sistema, planeja e, no `apply`, escreve.

## Modulos

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

## Interfaces pequenas, uma implementacao por caso

Partes que variam por SO ou por agente ficam atras de uma interface pequena, com uma implementacao por caso:

- **Agentes** (`src/agent`): cada adaptador traduz o modelo neutro de MCP (`src/mcp.rs`) para o formato do agente, le as entradas atuais e escreve pelo CLI do agente ou editando o arquivo.
- **Agendador** (`src/schedule`): Task Scheduler no Windows; timer do systemd de usuario ou crontab no Linux.
- **Shells** (`src/shell`): PowerShell 5.1 e 7, bash, zsh e fish.
- **Notificacao** (`src/notify`): toast do Windows via WinRT, `notify-send` no Linux.
- **Host** (`src/host`): Windows/Linux nativo ou Linux dentro do WSL, que ignora os executaveis do Windows.

Os testes trocam essas implementacoes por versoes em arquivo, por variaveis `AGSYNC_TEST_*`, e usam `examples/fake_agent.rs` no lugar dos CLIs reais.

## Fluxo de um apply

1. `Inputs::load` le `machine.toml`, `state.json` e os segredos.
2. `git ls-remote` resolve a `ref` do repo de config para um commit.
3. O snapshot desse commit vem do cache ou e baixado (`snapshots/`).
4. O `agsync.yaml` e validado; os `imports` dos perfis ativos sao resolvidos e juntados (`compose`).
5. As fontes git ativas sao resolvidas uma vez (`resolved`).
6. Para cada agente detectado, `desired` calcula o que ele deve ter e `plan` compara com o state e com o que o agente tem hoje.
7. O diff e os builds sao mostrados e confirmados.
8. Os builds rodam, cada adaptador faz backup e escreve item por item, e o state e gravado.
9. `pending.json` e limpo, `watch.json` e atualizado e caches sem uso sao removidos.

`status` e `diff` fazem os passos 1 a 6 e param. Veja [Planejamento e state](planejamento.md).

## Decisoes de design

- **CLI do agente primeiro.** Escrever pelo CLI segue o formato que o proprio agente mantem. Quando o CLI nao existe ou nao expressa a entrada, a edicao do arquivo passa pela arvore de sintaxe (JSONC ou TOML) e preserva comentarios e as outras chaves.
- **So o que escreveu.** O `state.json` separa entradas do agsync das manuais. Nada fora dele e removido, com uma unica excecao: o MCP `1mcp` no `migrate-legacy`.
- **Sem aplicar sozinho.** O `check` periodico so avisa; aplicar e sempre um comando explicito (ou `silentUpdate`/`--silent-update` no `init`).
- **Hook sem rede.** O shell abre rapido: o `hook` le arquivos locais e, no maximo, dispara um `check` em segundo plano.
- **`git` do sistema.** Credenciais, proxies e certificados ficam com a configuracao de git da maquina; o agsync nunca pede senha.
- **Commits sao imutaveis.** Snapshots, imports e fontes sao cacheados por url e commit e nunca ficam velhos; um `ls-remote` basta para saber se algo mudou.
- **Hash sem segredo.** A identidade de cada entrada no state nao depende dos valores dos segredos, entao trocar um segredo nao gera pendencia nem vaza no state.
- **Sem admin.** Instalacao, agendamento e hooks ficam no escopo do usuario.
