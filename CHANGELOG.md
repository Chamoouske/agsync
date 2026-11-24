# Changelog

Todas as mudancas relevantes do agsync ficam neste arquivo.

O formato segue o [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e as versoes seguem o [Versionamento Semantico](https://semver.org/lang/pt-BR/). As secoes novas sao geradas na release pelo [git-cliff](https://git-cliff.org/) a partir dos Conventional Commits: veja [Releases](https://chamoouske.github.io/agsync/contribuicao/releases.html#changelog).

## [0.14.2] - 2026-11-24

### Corrigido

- `promote` tenta ligar o auto-merge de novo quando os checks do PR de promocao acabaram de entrar na fila.

## [0.14.1] - 2026-11-24

### Corrigido

- `promote` e `sync-stage` fazem o merge direto de PRs que ja nascem prontos para merge.

## [0.14.0] - 2026-11-24

### Adicionado

- `agsync init` pergunta a URL do repo e os perfis quando nao sao passados.

## [0.13.0] - 2026-11-23

### Adicionado

- Suporte ao Antigravity CLI (`agy`) para MCPs, skills e plugins.
- Config de MCP em JSON que preserva comentarios e URLs de clone de plugins.

## [0.12.2] - 2026-11-18

### Corrigido

- Instalacoes pelo Scoop sao orientadas a rodar `scoop update` em vez de o agsync rodar.

## [0.12.1] - 2026-11-17

### Alterado

- Fluxo de branches com `stage`: promocao diaria de `stage` para `master` e volta automatica de `master` para `stage`.

## [0.12.0] - 2026-11-17

### Adicionado

- `agsync update`, que instala a release mais nova.
- Aviso quando existe uma release mais nova do agsync.

### Corrigido

- `agsync update` restaura a versao anterior no Linux quando a nova nao roda.
- `agsync update` so baixa por https, mantem o download em memoria e recusa plataformas sem binario publicado.
- Deteccao de instalacoes pelo Scoop em `SCOOP` e `SCOOP_GLOBAL`.
- Aceita so versoes `X.Y.Z` em ASCII, sem zeros a esquerda.

## [0.11.1] - 2026-11-17

### Alterado

- O envio ao `winget-pkgs` so roda com `WINGET_SUBMIT=true`; a instalacao pelo WinGet saiu da documentacao.

## [0.11.0] - 2026-11-17

### Adicionado

- Auto-merge do PR de manifests aberto pela release.

## [0.10.0] - 2026-11-17

### Alterado

- Os manifests sao atualizados depois da release, entao uma falha neles nao bloqueia a release.

## [0.9.0] - 2026-11-17

### Adicionado

- Cache em disco dos checkouts do repo de config por URL e commit: um commit sem mudanca nao e baixado de novo.

## [0.8.0] - 2026-11-17

### Adicionado

- Cache em disco dos `agsync.yaml` importados por URL e commit.

## [0.7.0] - 2026-11-17

### Adicionado

- `imports`: MCPs, skills e plugins vindos de outros repos de config, por perfil.
- MCPs importados so leem os segredos listados em `imports.mcps[].secrets`.
- Os relatorios mostram de qual import vem cada item.

### Corrigido

- Builds importados pedem confirmacao mesmo com `silentUpdate`.
- URLs e refs git que parecem opcoes, ou com credenciais, sao recusadas; o transporte `ext` fica desligado.

## [0.6.0] - 2026-11-15

### Adicionado

- Release publicada como `latest`, com PR de manifests do Scoop e do WinGet atualizados.

## [0.5.0] - 2026-11-14

### Adicionado

- `paths` em skills git, para listar varias skills de uma mesma fonte.

## [0.4.1] - 2026-11-14

### Alterado

- Ruleset de `master` versionado no repo, exigindo os checks do CI.

## [0.4.0] - 2026-11-14

### Adicionado

- `agsync apply --adopt` substitui instalacoes manuais com o mesmo nome; `diff`, `status`, `check` e a notificacao listam essas instalacoes.

## [0.3.0] - 2026-11-14

### Adicionado

- `agsync init --silent-update` e `silentUpdate` no `agsync.yaml`, para aplicar sem perguntar.

## [0.2.0] - 2026-11-14

### Adicionado

- Plugins vindos de um marketplace git, pelos CLIs dos agentes.
- Manifests do Scoop e do WinGet.
- Metadados de versao e manifest `asInvoker` no `agsync.exe`.

### Alterado

- Release publicada a cada push de codigo em `master`, com a versao calculada dos Conventional Commits.

### Corrigido

- Troca de ref de um marketplace do Claude restaura a ref antiga se falhar e preserva os escopos dos plugins.
- URLs de marketplace no formato scp com credenciais sao recusadas.
- `install.ps1` explica quando o `agsync.exe` falta ou foi bloqueado.

## [0.1.0] - 2026-11-14

### Adicionado

- Primeira versao: `init`, `status`, `diff`, `apply`, `check`, `hook`, `schedule`, `secret`, `doctor`, `rollback`, `uninstall` e `migrate-legacy`.
- Adaptadores para Claude Code, Codex, OpenCode e Hermes.
- MCPs e skills de fontes git, com build e cache por commit.
- Segredos em `secrets.env`, fora do `state.json`.
- `check` periodico pelo Agendador de Tarefas, timer do systemd ou cron, com notificacao do sistema.
- Instaladores `install.ps1` e `install.sh`.

[0.14.2]: https://github.com/Chamoouske/agsync/compare/v0.14.1...v0.14.2
[0.14.1]: https://github.com/Chamoouske/agsync/compare/v0.14.0...v0.14.1
[0.14.0]: https://github.com/Chamoouske/agsync/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/Chamoouske/agsync/compare/v0.12.2...v0.13.0
[0.12.2]: https://github.com/Chamoouske/agsync/compare/v0.12.1...v0.12.2
[0.12.1]: https://github.com/Chamoouske/agsync/compare/v0.12.0...v0.12.1
[0.12.0]: https://github.com/Chamoouske/agsync/compare/v0.11.1...v0.12.0
[0.11.1]: https://github.com/Chamoouske/agsync/compare/v0.11.0...v0.11.1
[0.11.0]: https://github.com/Chamoouske/agsync/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/Chamoouske/agsync/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/Chamoouske/agsync/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/Chamoouske/agsync/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/Chamoouske/agsync/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/Chamoouske/agsync/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/Chamoouske/agsync/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/Chamoouske/agsync/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/Chamoouske/agsync/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/Chamoouske/agsync/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Chamoouske/agsync/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Chamoouske/agsync/releases/tag/v0.1.0
