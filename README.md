# agsync

O agsync aplica MCP servers e skills de um repositorio git de configuracao (o `agsync.yaml`) na configuracao de usuario dos agentes de codigo da maquina: Claude Code, Codex, OpenCode e Hermes.

- So mexe no que ele mesmo escreveu. Isso fica registrado em `state.json`. Entradas manuais com o mesmo nome viram conflito e ficam como estao, a nao ser com `--adopt`.
- Escreve pelo CLI do agente quando o CLI sabe fazer a mudanca. Senao, edita o arquivo preservando comentarios.
- Faz backup antes de cada mudanca.
- Um `agsync check` periodico so le o remoto e avisa. Nada e aplicado sem `agsync apply`.

## Instalacao

Windows (PowerShell, sem admin). Instala em `%LOCALAPPDATA%\agsync\bin` e poe essa pasta no PATH do usuario:

```powershell
irm https://raw.githubusercontent.com/Chamoouske/agsync/main/install.ps1 | iex
```

Uma versao especifica:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/Chamoouske/agsync/main/install.ps1))) -Version 0.1.0
```

Nao use `-ExecutionPolicy Bypass`. O `irm ... | iex` roda sem mudar a politica de execucao, e o Defender costuma marcar justamente essa linha como suspeita. Se algo der errado, veja [Microsoft Defender e SmartScreen](#microsoft-defender-e-smartscreen).

Via Scoop, pelo manifest do proprio repositorio (`packaging/scoop`, que nao esta no formato de bucket, entao o Scoop instala direto pela URL):

```powershell
scoop install https://raw.githubusercontent.com/Chamoouske/agsync/main/packaging/scoop/agsync.json
```

O manifest do Winget esta em `packaging/winget`, pronto para um PR em `microsoft/winget-pkgs`. Enquanto esse PR nao for aceito, `winget install` nao encontra o agsync.

Linux (x86_64, binario estatico). Instala em `~/.local/bin` e, se preciso, poe essa pasta no PATH pelo `~/.profile` e pelo `~/.bashrc`/`~/.zshrc` que existirem:

```sh
curl -fsSL https://raw.githubusercontent.com/Chamoouske/agsync/main/install.sh | sh
curl -fsSL https://raw.githubusercontent.com/Chamoouske/agsync/main/install.sh | sh -s -- --version 0.1.0
```

Os dois scripts baixam o binario do GitHub Releases e conferem o SHA256 com o `SHA256SUMS` da release. Os binarios nao tem assinatura de codigo.

Alternativa, com Rust instalado:

```sh
cargo install --git https://github.com/Chamoouske/agsync --tag v0.1.0
```

Requisito: `git` no PATH.

## Primeiro uso

```sh
agsync init https://github.com/<org>/agent-config --profile work
```

O `init`:

1. grava o `machine.toml`;
2. testa o acesso ao repo;
3. registra o `agsync check` periodico;
4. oferece o hook em cada shell;
5. roda o primeiro `apply`.

Opcoes:

- `--ref <branch|tag|sha>`: o padrao e `main`.
- `--profile` pode repetir.
- `--yes`: instala o hook e aplica sem perguntar.

Repo privado: o `git` precisa de credencial propria, como chave SSH ou credential helper. O agsync nunca pede senha.

## agsync.yaml

Fica na raiz do repo de config.

```yaml
version: 1
profiles: [work, personal]

mcps:
  context7:
    url: https://mcp.context7.com/mcp
    headers:
      CONTEXT7_API_KEY: ${CONTEXT7_API_KEY}

  filesystem:
    command: npx
    args: ["-y", "@modelcontextprotocol/server-filesystem", "/home/me/src"]
    env: {LOG_LEVEL: info}
    agents: [claude, opencode]

  internal-tool:
    profiles: [work]
    command: node
    args: ["{dir}/dist/index.js"]
    env: {TOKEN: "${INTERNAL_TOKEN}"}
    source:
      git: https://github.com/<org>/internal-tool
      ref: v1.2.0
      requires: [node, npm]
      build: ["npm ci", "npm run build"]

skills:
  - path: skills/grug-brain-code
  - git: https://github.com/mattpocock/skills
    ref: 4ae2317ddb22b046e26455e4f9c9b3aaaf62212d
    path: skills/engineering/tdd
  - git: https://github.com/<org>/more-skills
    ref: main
    path: review
    name: team-review
    profiles: [work]
    agents: [claude]

plugins:
  - name: code-review
    marketplace:
      git: https://gitlab.example.com/group/marketplace.git
      ref: v1.4.0
  - name: team-review
    profiles: [work]
    agents: [claude, opencode]
    marketplace:
      git: https://gitlab.example.com/group/marketplace.git
      ref: v1.4.0
```

Campos:

- `version`: sempre `1`.
- `profiles`: todos os perfis que os itens podem citar.
- `mcps.<nome>`: o nome aceita letras, numeros, `-` e `_`.
  - stdio: `command`, com `args` e `env` opcionais.
  - remoto (HTTP): `url`, com `headers` opcional.
  - Defina exatamente um entre `command` e `url`.
  - `profiles`: o item vale so nesses perfis. Sem `profiles`, vale em todos.
  - `agents`: o item vale so nesses agentes (`claude`, `codex`, `opencode`, `hermes`). Sem `agents`, vale em todos.
  - `source`: o servidor e construido de um repo git.
    - `git` e `ref` sao obrigatorios. O `ref` aceita branch, tag ou SHA completo; SHA abreviado e recusado.
    - `requires`: programas que precisam estar no PATH. Se faltar algum, o item e pulado.
    - `build`: comandos rodados no checkout, por `sh -c` no Linux e `cmd /c` no Windows.
    - `{dir}` em `command`, `args` e `env` vira a pasta do checkout.
- `skills[]`: cada item e uma pasta com `SKILL.md`.
  - `path`: pasta no repo de config ou, com `git` e `ref`, pasta dentro de outro repo.
  - `name`: o padrao e o ultimo segmento do `path`.
  - `profiles` e `agents`: como nos MCPs.
- `plugins[]`: um plugin publicado em um marketplace git.
  - `name`: o nome declarado no catalogo do marketplace.
  - `marketplace.git` e `marketplace.ref`: obrigatorios. O `ref` aceita branch, tag ou SHA completo; SHA abreviado e recusado. Um marketplace pode ter varias entradas, sempre com a mesma `ref`.
  - `profiles` e `agents`: como nos MCPs.
  - Credenciais ficam no credential helper do git. URL com senha ou token embutido e recusada.
  - Veja [Plugins](#plugins) para o suporte de cada agente.
- `${VAR}`: aceito em `command`, `args`, `env`, `url` e `headers`. Veja [Segredos](#segredos).

Uma fonte em branch ou tag e seguida: um commit novo nela tambem aparece no `check`.

## Comandos

| comando | o que faz |
|---|---|
| `agsync init <url> --profile P` | configura a maquina (veja acima) |
| `agsync status` | commit aplicado e remoto, mudancas pendentes, desvios, conflitos e itens pulados |
| `agsync diff` | o que o proximo `apply` muda, por agente |
| `agsync apply [--yes] [--adopt]` | mostra o diff e os builds, pede confirmacao e aplica; `--adopt` assume entradas manuais com o mesmo nome |
| `agsync rollback [--yes]` | reaplica o commit aplicado antes do atual, pelo mesmo fluxo de plano e apply |
| `agsync check` | so le o remoto: grava `pending.json` e notifica uma vez por novidade; roda pelo agendador |
| `agsync hook` | imprime a linha de pendencia, sem rede; roda pelo perfil do shell |
| `agsync hook install [--yes]` / `hook remove` | poe ou tira o hook em PowerShell, pwsh, bash, zsh e fish |
| `agsync schedule install\|remove\|status` | o `check` periodico: Agendador de Tarefas no Windows, timer do systemd de usuario ou cron no Linux |
| `agsync secret set NOME` / `secret list` | grava um valor no `secrets.env`, ou lista os nomes |
| `agsync doctor` | confere git, repo, agentes, requires, segredos, agendamento e repos de fonte |
| `agsync migrate-legacy [--yes]` | backup e remocao do sistema antigo (veja [Migracao](#migracao-a-partir-do-agent-config)) |
| `agsync uninstall [--yes] [--purge]` | tira dos agentes tudo o que esta no state, remove agendamento e hooks e apaga `state.json` e `pending.json` |

Sem terminal, `apply`, `rollback`, `migrate-legacy` e `uninstall` so rodam com `--yes`.

Depois de um `rollback`, o `check` volta a acusar o commit remoto como pendente. Isso e esperado: rode `agsync apply` para voltar a ele.

O `uninstall --purge` apaga tambem as pastas de config e de dados: `secrets.env`, cache e **todos os backups**. O binario nunca e removido; o comando imprime onde ele esta.

Pastas locais:

| | Windows | Linux |
|---|---|---|
| config (`machine.toml`, `secrets.env`) | `%APPDATA%\agsync` | `~/.config/agsync` |
| dados (state, cache, backups, logs) | `%LOCALAPPDATA%\agsync` | `~/.local/share/agsync` |

As pastas podem ser trocadas por `AGSYNC_CONFIG_DIR` e `AGSYNC_DATA_DIR`.

## Segredos

Um `${VAR}` e resolvido primeiro pelo ambiente do processo e depois pelo `secrets.env`.

- `agsync secret set NOME` le o valor sem eco, ou da stdin. O arquivo e acessivel so pelo usuario.
- Valor vindo do ambiente: o agente que expande variaveis recebe a referencia `${VAR}`, nao o valor.
  - O Codex so expande em alguns campos: env com o proprio nome, header `${VAR}` e `Authorization: Bearer ${VAR}`. Nos demais campos ele recebe o valor.
- Valor so no `secrets.env`: o agente recebe o valor.
- Segredo ausente: o item e pulado e aparece em `status` e `doctor`.
- Valores de `env` e `headers` nunca aparecem na saida do agsync.
- O hash do `state.json` nao depende dos valores.

## Perfis

- O `agsync.yaml` declara os perfis possiveis.
- Cada maquina escolhe os seus no `init` (`--profile work --profile personal`). Eles ficam no `machine.toml`.
- Um item com `profiles` so vale nas maquinas com um desses perfis ativos. Um item sem `profiles` vale em todas.
- Um item que sai do perfil ativo sai dos agentes no proximo `apply`.

## Agentes

Um agente e detectado pelo executavel no PATH. Um agente ausente e pulado e aparece em `status`.

- **Claude Code**:
  - MCPs: `claude mcp add-json/remove --scope user`. O `.claude.json` nunca e editado pelo agsync.
  - Skills: `~/.claude/skills`.
  - Com `CLAUDE_CONFIG_DIR`, as duas coisas ficam nessa pasta.
- **Codex**:
  - MCPs: `codex mcp add/remove` quando o CLI expressa a entrada. Senao, o agsync edita `$CODEX_HOME/config.toml` mantendo comentarios.
  - Skills: `~/.agents/skills`.
- **OpenCode**:
  - MCPs: `opencode mcp add --global`. O OpenCode nao tem `mcp remove`, entao a remocao edita o `opencode.json(c)` mantendo comentarios.
  - Skills: `~/.config/opencode/skills`.
  - O OpenCode tambem le `~/.claude/skills` e `~/.agents/skills`. O `status` avisa quando uma skill restrita por `agents` fica visivel nele por isso.
  - Depois do apply: `opencode reload`.
- **Hermes**:
  - MCPs: `hermes config set/unset mcp_servers.<nome>`.
  - Skills: `<HERMES_HOME>/skills`.

Skills sao copiadas para um cache em `<dados>/skills` e ligadas no agente. A ordem de tentativa e symlink, depois junction (Windows), depois copia.

MCPs so valem depois de reiniciar o agente.

## Plugins

So o que foi instalado pelo agsync e registrado no `state.json`. Plugin instalado a mao nunca e alterado nem removido.

| agente | suporte | como |
|---|---|---|
| Claude Code | sim | `claude plugin marketplace add` e `plugin install/update/uninstall` |
| OpenCode | sim | `opencode plugin add/remove`, gravando o array `plugins` |
| Codex | nao | o CLI nao instala plugin de forma nao interativa; o item e pulado com o motivo no `status` |
| Hermes | nao | idem |

- O `check` acusa plugin adicionado, com `ref` nova e removido do `agsync.yaml`.
- O `diff` e a confirmacao do `apply` listam as mudancas de plugin por agente.
- Plugin que sai do `agsync.yaml` e desinstalado no `apply` seguinte.
- Plugin de perfil inativo nao e baixado nem instalado.
- O `diff` avisa quando um plugin tem o mesmo nome de um MCP ou skill do mesmo agente: os dois podem aparecer lado a lado no agente.
- O `uninstall` desinstala os plugins do `state.json`; os dados do plugin ficam no disco (`--keep-data` no Claude).

Limitacoes:

- Marketplace em pasta local: o Claude le a pasta direto e nao aceita `ref` nela. A `ref` continua na identidade do plugin, entao troca-la ainda gera um `update`.
- Trocar a `ref` de um marketplace remoto: o Claude nao troca a `ref` no lugar, e remover o marketplace desinstala todos os plugins dele. O agsync remove e readiciona o marketplace e reinstala o que gerencia. Se houver plugin fora do agsync no mesmo marketplace, ele recusa e pede a troca manual.
- O marketplace em si fica registrado no Claude; o agsync nao remove marketplaces no `uninstall`.

## WSL

O agsync no WSL e uma instalacao Linux separada. Ele cuida das configs do home Linux; o Windows precisa da sua propria instalacao.

- Programas do Windows vistos pelo interop (em `/mnt/<letra>/`) sao ignorados como agentes, `requires` e `git`. O `status` e o `doctor` dizem quando um agente so existe no Windows.
- Notificacoes ficam so no terminal, pelo `agsync hook` e pelo `agsync status`.
- Sem systemd e sem cron, o `agsync hook` dispara o `check` em segundo plano quando o shell abre.
- O PATH gravado no agendamento ignora as pastas do Windows.

## Seguranca

- `source.build` executa comandos de terceiros na sua maquina. Na pratica, quem escreve no repo de config executa codigo em todas as maquinas que aplicam esse repo.
- Proteja a branch seguida pelo agsync com revisao obrigatoria de pull request.
- A confirmacao do `apply` lista cada build, com repo, commit e comandos, antes de rodar qualquer coisa. Leia antes de confirmar.
- Fixe `ref` por SHA completo quando nao quiser seguir uma branch.
- O `check` nunca aplica nada, e o `hook` nunca acessa a rede.

## Microsoft Defender e SmartScreen

O binario do Windows nao tem assinatura de codigo, nem metadados de versao, e chama o PowerShell por dentro. Por isso o classificador de aprendizado de maquina do Microsoft Defender pode poe o `agsync.exe` em quarentena (por exemplo, como `Trojan:Win32/Bearfoos.A!ml`) ou o SmartScreen pode avisar sobre um aplicativo desconhecido. E um falso positivo conhecido da v0.1.0, mas confira antes de liberar.

1. Confira o SHA256 do arquivo contra o `SHA256SUMS` da release:

```powershell
(Get-FileHash "$env:LOCALAPPDATAgsyncingsync.exe" -Algorithm SHA256).Hash.ToLower()
```

Compare com a linha de `agsync-x86_64-pc-windows-msvc.exe` no `SHA256SUMS` da [release](https://github.com/Chamoouske/agsync/releases). Se o hash for diferente, nao use o arquivo. O `install.ps1` ja faz essa conferencia antes de copiar o binario.

2. Para restaurar da quarentena: Seguranca do Windows > Protecao contra virus e ameacas > Historico de protecao, escolha a deteccao do `agsync.exe` e use Acoes > Restaurar. Depois rode o instalador de novo se o arquivo tiver sumido.

3. Se o Defender voltar a bloquear, exclua so o arquivo, nunca a pasta inteira, em um PowerShell como administrador:

```powershell
Add-MpPreference -ExclusionPath "$env:LOCALAPPDATAgsyncingsync.exe"
```

Risco: o Defender deixa de examinar esse arquivo. Faca isso so depois de conferir o SHA256, e remova a exclusao com `Remove-MpPreference -ExclusionPath` se trocar de binario por um que voce nao conferiu.

4. Para ajudar a corrigir a deteccao, envie o arquivo como falso positivo em <https://www.microsoft.com/wdsi/filesubmission>.

## Migracao a partir do agent-config

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
