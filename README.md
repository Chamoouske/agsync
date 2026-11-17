# agsync

O agsync aplica MCP servers, skills e plugins de um repositorio git de configuracao (o `agsync.yaml`) na configuracao de usuario dos agentes de codigo da maquina: Claude Code, Codex, OpenCode e Hermes. O repo de config pode importar itens de outros repos, por perfil.

- So mexe no que ele mesmo escreveu. Isso fica registrado em `state.json`. Entradas manuais com o mesmo nome viram conflito e ficam como estao, a nao ser com `--adopt`.
- Escreve pelo CLI do agente quando o CLI sabe fazer a mudanca. Senao, edita o arquivo preservando comentarios.
- Faz backup antes de cada mudanca.
- Um `agsync check` periodico so le o remoto e avisa. Nada e aplicado sem `agsync apply`.

## Instalacao

Windows (PowerShell, sem admin). Instala em `%LOCALAPPDATA%\agsync\bin` e poe essa pasta no PATH do usuario:

```powershell
irm https://raw.githubusercontent.com/Chamoouske/agsync/master/install.ps1 | iex
```

Uma versao especifica:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/Chamoouske/agsync/master/install.ps1))) -Version 0.1.0
```

Nao use `-ExecutionPolicy Bypass`. O `irm ... | iex` roda sem mudar a politica de execucao, e o Defender costuma marcar justamente essa linha como suspeita. Se algo der errado, veja [Microsoft Defender e SmartScreen](#microsoft-defender-e-smartscreen).

Via Scoop, pelo manifest anexado a ultima release (o mesmo de `packaging/scoop`, que nao esta no formato de bucket, entao o Scoop instala direto pela URL):

```powershell
scoop install https://github.com/Chamoouske/agsync/releases/latest/download/agsync.json
```

O manifest de `master` (`https://raw.githubusercontent.com/Chamoouske/agsync/master/packaging/scoop/agsync.json`) tambem funciona, mas so aponta para a versao nova depois do merge do PR de manifests (veja [Publicacao de releases](#publicacao-de-releases)).

O manifest do Winget esta em `packaging/winget`, pronto para um PR em `microsoft/winget-pkgs`. Enquanto esse PR nao for aceito, `winget install` nao encontra o agsync.

Linux (x86_64, binario estatico). Instala em `~/.local/bin` e, se preciso, poe essa pasta no PATH pelo `~/.profile` e pelo `~/.bashrc`/`~/.zshrc` que existirem:

```sh
curl -fsSL https://raw.githubusercontent.com/Chamoouske/agsync/master/install.sh | sh
curl -fsSL https://raw.githubusercontent.com/Chamoouske/agsync/master/install.sh | sh -s -- --version 0.1.0
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
- `--silent-update`: aplica as atualizacoes pendentes sem perguntar, como `agsync apply --yes`, tambem sem terminal. O hook de shell continua perguntando. O resultado (aplicados, pulados, falhas) continua impresso, e uma falha faz o `init` sair com erro.
- `silentUpdate: true` no `agsync.yaml` tem o mesmo efeito para toda maquina que seguir o repo. Sem a opcao, vale o fluxo interativo.
- Excecao: com build de MCP vindo de um import, `--silent-update` e `silentUpdate` nao dispensam a confirmacao. O `init` pergunta como no fluxo interativo; sem terminal, nada e aplicado e o motivo aparece. So `--yes` explicito aplica sem perguntar.

Repo privado: o `git` precisa de credencial propria, como chave SSH ou credential helper. O agsync nunca pede senha.

## agsync.yaml

Fica na raiz do repo de config.

```yaml
version: 1
profiles: [work, personal]
silentUpdate: false   # opcional; true faz o `init` aplicar sem perguntar

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
    paths:
      - skills/engineering/tdd
      - skills/engineering/pr
      - skills/productivity/writing-for-agents
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

imports:
  skills:
    - git: https://github.com/<org>/skills-personal
      ref: main
      profiles: [personal]
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
    - `git` e `ref` sao obrigatorios. O `ref` aceita branch, tag ou SHA completo; SHA abreviado e recusado. URL com senha ou token embutido e recusada.
    - `requires`: programas que precisam estar no PATH. Se faltar algum, o item e pulado.
    - `build`: comandos rodados no checkout, por `sh -c` no Linux e `cmd /c` no Windows.
    - `{dir}` em `command`, `args` e `env` vira a pasta do checkout.
- `skills[]`: cada item e uma pasta com `SKILL.md`.
  - `path`: pasta no repo de config ou, com `git` e `ref`, pasta dentro de outro repo. URL `git` com senha ou token embutido e recusada.
  - `paths`: com `git` e `ref`, lista de pastas do mesmo repo. Cada pasta vira uma skill independente, com o nome do ultimo segmento, e todas compartilham `git`, `ref`, `profiles` e `agents`. O ref e resolvido e o repo baixado uma vez so. Use `path` ou `paths`, nunca os dois; `name` nao vale com `paths`. Uma pasta sem `SKILL.md` no repo e pulada e reportada, sem bloquear as outras.
  - `name`: o padrao e o ultimo segmento do `path`.
  - `profiles` e `agents`: como nos MCPs.
- `plugins[]`: um plugin publicado em um marketplace git.
  - `name`: o nome declarado no catalogo do marketplace.
  - `marketplace.git` e `marketplace.ref`: obrigatorios. O `ref` aceita branch, tag ou SHA completo; SHA abreviado e recusado. Um marketplace pode ter varias entradas, sempre com a mesma `ref`.
  - `profiles` e `agents`: como nos MCPs.
  - Credenciais ficam no credential helper do git. URL com senha ou token embutido e recusada.
  - Veja [Plugins](#plugins) para o suporte de cada agente.
- `imports`: opcional. Traz MCPs, skills ou plugins do `agsync.yaml` de outros repos. Veja [Composicao entre repositorios](#composicao-entre-repositorios).
- `${VAR}`: aceito em `command`, `args`, `env`, `url` e `headers`. Veja [Segredos](#segredos).
- Todo campo `git` e `ref` (`source`, `skills`, `marketplace` e `imports`): valor que comeca com `-` e recusado, porque o `git` o leria como opcao.

Uma fonte em branch ou tag e seguida: um commit novo nela tambem aparece no `check`.

## Comandos

| comando | o que faz |
|---|---|
| `agsync init <url> --profile P` | configura a maquina (veja acima) |
| `agsync status` | commit aplicado e remoto, mudancas pendentes, desvios, conflitos e itens pulados |
| `agsync diff` | o que o proximo `apply` muda, por agente |
| `agsync apply [--yes] [--adopt]` | mostra o diff e os builds, pede confirmacao e aplica; `--adopt` substitui instalacoes manuais com o mesmo nome (veja abaixo) |
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

### Instalacoes manuais com o mesmo nome

Um MCP, skill ou plugin instalado a mao que tem exatamente o mesmo nome, o mesmo tipo e o mesmo agente de um item do `agsync.yaml` e uma correspondencia:

- `status` e `diff` mostram o item como `conflito` e listam todas as correspondencias no fim;
- `check` registra as correspondencias no `pending.json` (`conflicts`) e na notificacao;
- `apply` sem `--adopt` nao mexe nelas;
- `apply --adopt` lista, antes da confirmacao, as instalacoes manuais que serao substituidas, troca cada uma pela definicao do `agsync.yaml` (origem, versao/ref e configuracao) com backup e passa a gerencia-la no `state.json`.

Nome diferente, outro tipo (uma skill `x` nao corresponde a um MCP `x`) ou outro agente nunca contam como correspondencia. Se a substituicao de um item falha, o erro e mostrado, o item nao entra no `state.json` e continua como conflito no proximo `apply`.

Plugins: no Claude Code, um plugin manual com o mesmo nome vindo de outro marketplace tambem conta, se for o unico com esse nome; o `--adopt` instala o do `agsync.yaml` e desinstala o manual. No OpenCode so conta o mesmo pacote.

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
- MCP importado so le os segredos listados em `imports.mcps[].secrets`. Veja [Composicao entre repositorios](#composicao-entre-repositorios).
- Valores de `env` e `headers` nunca aparecem na saida do agsync.
- O hash do `state.json` nao depende dos valores.

## Perfis

- O `agsync.yaml` declara os perfis possiveis.
- Cada maquina escolhe os seus no `init` (`--profile work --profile personal`). Eles ficam no `machine.toml`.
- Um item com `profiles` so vale nas maquinas com um desses perfis ativos. Um item sem `profiles` vale em todas.
- Um item que sai do perfil ativo sai dos agentes no proximo `apply`.

## Composicao entre repositorios

O campo `imports` do `agsync.yaml` junta itens de outros repos de config ao repo principal. Cada import pode valer so em alguns perfis.

```yaml
imports:
  mcps:
    - git: https://github.com/<org>/mcps-work
      ref: main
      profiles: [work]
      excludes: [mcp-test]
      secrets: [GITHUB_TOKEN]
  skills:
    - git: https://github.com/<org>/skills-personal
      ref: 8f3a2c1e5b7d9f0a1c3e5b7d9f0a1c3e5b7d9f0a
      profiles: [personal]
      excludes: [tdd]
  plugins:
    - git: https://github.com/<org>/plugins-work
      ref: v1.2.0
      profiles: [work]
      excludes: [plugin-test]
```

Cada entrada importa so um tipo (`mcps`, `skills` ou `plugins`) do `agsync.yaml` na raiz do repo referenciado. O mesmo repo pode aparecer em mais de um tipo.

Campos:

- `git`: obrigatorio. URL com senha ou token embutido e recusada; credenciais ficam no credential helper do git. URL que comeca com `-` e recusada.
- `ref`: branch, tag ou SHA completo; SHA abreviado e ref que comeca com `-` sao recusados.
  - Sem `ref`, vale `main`, o mesmo padrao do `agsync init`.
  - O conteudo importado e exatamente o do commit para onde o `ref` aponta.
  - Ref por branch ou tag e seguido: um commit novo no repo importado aparece no `check`.
- `profiles`: o import so vale nas maquinas com um desses perfis ativos. Sem `profiles`, vale em todas.
  - Import de perfil inativo nem e acessado na rede.
  - Os itens importados assumem os `profiles` da entrada de import. Os `profiles` declarados nos itens do repo importado sao ignorados.
  - Os `agents` dos itens importados sao mantidos.
- `excludes`: nomes de recursos daquele tipo, daquela origem, que nao entram.
  - Nao afeta recurso de mesmo nome do repo principal nem de outro import.
  - Nome que nao existe na origem gera aviso.
- `secrets`: so em `imports.mcps`; em `skills` e `plugins` e erro. Nomes de variavel que os MCPs dessa origem podem usar como `${VAR}`.
  - Sem `secrets`, nenhum MCP importado le segredo.
  - MCP importado com `${VAR}` fora da lista, em `command`, `args`, `env`, `url` ou `headers`, e pulado. A mensagem nos pulados diz a variavel e como liberar: incluir o nome em `imports.mcps[].secrets`.

```yaml
imports:
  mcps:
    - git: https://github.com/<org>/mcps-work
      secrets: [GITHUB_TOKEN]   # o MCP importado pode usar ${GITHUB_TOKEN}
```

O que vem do repo importado:

- Skill em pasta local do repo importado e baixada do proprio repo importado, no commit resolvido.
- Plugin com marketplace em pasta local e pulado e aparece nos pulados.
- Skill com `git` ou MCP com `source.git` apontando para caminho local e pulado e aparece nos pulados: o caminho so faz sentido no repo de origem.
- So um nivel: os `imports` do repo importado nao sao seguidos, com aviso.
- `silentUpdate` e `profiles` do repo importado sao ignorados.

Conflito de nomes (mesmo tipo, entre itens ativos na maquina):

- O repo principal vence qualquer import.
- Entre imports, vence o declarado primeiro em `imports.<tipo>`.
- O perdedor e descartado com aviso nomeando as duas origens.
- Item do principal de perfil inativo nao conflita.
- Exemplo: o perfil `personal` importa a skill `tdd` de um repo e o `work` importa `tdd` de outro. Numa maquina so com `personal` nao ha conflito. Com os dois perfis ativos, vence o import declarado primeiro.

Na confirmacao:

- `status`, `diff` e a confirmacao do `apply` mostram a origem de cada MCP, skill e plugin importado, por exemplo `(de imports.mcps https://github.com/<org>/mcps-work@main)`. Itens do repo principal aparecem sem origem.
- A lista de builds tambem mostra a origem do build de MCP importado.
- Build de MCP importado sempre pede confirmacao: `--silent-update` e `silentUpdate` nao valem nesse caso. `apply --yes` explicito continua valendo.

Erros:

- Repo inacessivel, ref inexistente ou `agsync.yaml` ausente ou invalido no repo importado fazem `status`, `diff`, `apply` e `check` falharem.
- A mensagem nomeia o tipo, o `git` e o `ref` do import.
- Nesse caso nada e aplicado nem removido.

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

So o que foi instalado pelo agsync e registrado no `state.json`. Plugin instalado a mao so e alterado com `apply --adopt`, quando tem o mesmo nome de um plugin do `agsync.yaml` (veja [Instalacoes manuais com o mesmo nome](#instalacoes-manuais-com-o-mesmo-nome)).

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
- Trocar a `ref` de um marketplace remoto: o Claude nao troca a `ref` no lugar, e remover o marketplace desinstala todos os plugins dele. O agsync remove e readiciona o marketplace e reinstala o que gerencia, cada plugin no seu escopo. Se a `ref` nova nao puder ser adicionada, ele readiciona a `ref` antiga com os plugins e diz o que nao voltou. Se houver plugin fora do agsync no mesmo marketplace, ele recusa e pede a troca manual.
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
- Um repo em `imports` tambem vira quem escreve na sua maquina. Seus MCPs so leem os segredos de `imports.mcps[].secrets`, seus builds sempre pedem confirmacao, e a origem de cada item importado aparece no `diff`.
- Valor de `git` ou `ref` que comeca com `-` e recusado, e o agsync chama o `git` com `--end-of-options` e com o transporte `ext::` desligado.
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

## Contribuindo: CI obrigatoria

Toda mudanca entra em `master` por PR. O ruleset `master` do GitHub ([`.github/rulesets/master.json`](.github/rulesets/master.json)) exige:

- os checks `check (ubuntu-latest)` e `check (windows-latest)` do workflow `ci` (`cargo fmt --check`, `cargo clippy -D warnings` e `cargo test`), vindos do GitHub Actions;
- a branch do PR atualizada com `master` antes do merge;
- nenhum push direto, force push ou remocao de `master`.

Enquanto um check obrigatorio estiver pendente, rodando ou falho, o botao de merge fica bloqueado. Nao ha excecao por autor ou aprovador, nem para admins: o ruleset nao tem bypass. Se um check falhar, corrija na branch e faca push; o CI roda de novo. Se `master` andar, atualize a branch ("Update branch") e espere o CI.

Admins do repo podem editar ou desativar o ruleset em Settings > Rules. Para reaplicar a versao do repo:

```bash
gh api -X PUT repos/Chamoouske/agsync/rulesets/27269112 --input .github/rulesets/master.json
```

## Publicacao de releases

Cada push em `master` (fora `*.md`, `docs/`, `LICENSE*` e `packaging/`) roda o workflow `release`:

1. `ci` e os builds de Windows e Linux; a versao sai dos Conventional Commits desde a ultima tag (`scripts/next-version.sh`).
2. O job `publish` gera o `SHA256SUMS` dos binarios e roda `scripts/update-manifests.sh <versao> dist/SHA256SUMS`, que reescreve versao, URL e hash em `packaging/scoop/agsync.json` e nos tres arquivos de `packaging/winget` (o hash do WinGet em maiusculas, o `autoupdate` do Scoop intacto).
3. `scripts/check-manifests.sh dist/SHA256SUMS` confere que os quatro arquivos tem a mesma versao, o `PackageIdentifier` `Chamoouske.agsync`, as URLs da tag `v<versao>` e o hash do `SHA256SUMS`. `scripts/validate-manifest-schemas.sh` valida contra o schema do Scoop e os schemas 1.6.0 do WinGet.
4. A release `v<versao>` e publicada como `latest` (`make_latest`), com os binarios, o `SHA256SUMS` e o `agsync.json` do Scoop. O job confere que `releases/latest` e essa tag e roda `scripts/check-manifests.sh --remote`, que baixa o binario publicado e compara o hash.
5. Os manifests atualizados vao para a branch `release/manifests-v<versao>` e o job abre um PR para `master`. O merge desse PR nao dispara outra release, porque `packaging/` esta no `paths-ignore`.

O workflow `ci` tambem tem o job `manifests`: roda `scripts/test-manifests.sh` (manifest desatualizado, URL de outra versao, hash divergente do `SHA256SUMS`, do outro manifest ou do artefato publicado), confere os manifests de `master` contra o binario publicado e valida os schemas.

Configuracao externa:

- Secret `RELEASE_PR_TOKEN`: token (fine-grained, com Contents e Pull requests de escrita neste repo) usado para abrir o PR de manifests. Sem ele o job usa o `GITHUB_TOKEN`, e o GitHub nao roda o `ci` em PR aberto por esse token, entao os checks obrigatorios ficam pendentes; feche e reabra o PR ou faca um push nele para rodar o CI. Sem o secret, abrir o PR tambem exige Settings > Actions > General > "Allow GitHub Actions to create and approve pull requests"; se nem isso estiver ligado, o job deixa um aviso com o link para abrir o PR na mao.
- WinGet: `packaging/winget` e so a fonte. Publicar em `winget install` ainda depende de um PR em `microsoft/winget-pkgs` com esses arquivos (por exemplo com `wingetcreate submit`), que nao esta automatizado.

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
