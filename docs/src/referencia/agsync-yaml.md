# agsync.yaml

Fica na raiz do repo de config. O agsync valida o arquivo inteiro antes de planejar; campos desconhecidos sao erro, e cada erro aponta a linha.

## Exemplo completo

```yaml
version: 1
profiles: [work, personal]
silentUpdate: false

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

imports:
  skills:
    - git: https://github.com/<org>/skills-personal
      ref: main
      profiles: [personal]
```

## Raiz

| campo | tipo | obrigatorio | descricao |
|---|---|---|---|
| `version` | inteiro | sim | sempre `1` |
| `profiles` | lista de nomes | nao | todos os perfis que os itens podem citar |
| `silentUpdate` | booleano | nao | `true` faz o `agsync init` aplicar sem perguntar, como `--silent-update`; padrao `false` |
| `mcps` | mapa nome -> MCP | nao | MCP servers |
| `skills` | lista | nao | skills |
| `plugins` | lista | nao | plugins de marketplaces |
| `imports` | objeto | nao | itens de outros repos de config |

## Regras comuns

- **Nomes** de MCP, skill, plugin e `excludes`: letras, numeros, `-` e `_`.
- **`profiles`** de um item: cada perfil precisa estar em `profiles` da raiz. Sem `profiles` (ou lista vazia), o item vale em todos os perfis.
- **`agents`** de um item: `claude`, `codex`, `opencode`, `hermes`, `antigravity`. Sem `agents`, vale em todos. Lista vazia e erro.
- **`git`**: URL, atalho do GitHub ou pasta local, conforme o campo. URL com senha ou token embutido e recusada (exceto o usuario de `ssh://git@host`); use o credential helper do git. Valor que comeca com `-` e recusado.
- **`ref`**: branch, tag ou SHA completo. SHA abreviado e valor que comeca com `-` sao recusados. Branch e tag sao seguidas: um commit novo aparece no `check`.
- **`${VAR}`**: aceito em `command`, `args`, `env`, `url` e `headers`. Veja [Segredos](../guias/segredos.md).

## mcps.&lt;nome&gt;

| campo | tipo | descricao |
|---|---|---|
| `command` | texto | executavel de um MCP stdio |
| `args` | lista de texto | argumentos; so com `command` |
| `env` | mapa | variaveis de ambiente; so com `command` |
| `url` | texto | endereco de um MCP remoto (HTTP) |
| `headers` | mapa | cabecalhos HTTP; so com `url` |
| `profiles` | lista | perfis em que o item vale |
| `agents` | lista | agentes em que o item vale |
| `source` | objeto | construir o servidor de um repo git |

Defina exatamente um entre `command` e `url`.

### mcps.&lt;nome&gt;.source

| campo | tipo | obrigatorio | descricao |
|---|---|---|---|
| `git` | texto | sim | repo do servidor |
| `ref` | texto | sim | branch, tag ou SHA completo |
| `requires` | lista | nao | programas que precisam estar no PATH; se faltar algum, o item e pulado |
| `build` | lista | nao | comandos rodados no checkout, por `sh -c` no Linux e `cmd /c` no Windows |

`{dir}` em `command`, `args` e `env` vira a pasta do checkout. Veja [MCP construido de um repo git](../guias/mcp-de-fonte-git.md).

## skills[]

Cada item e uma pasta com `SKILL.md`.

| campo | tipo | descricao |
|---|---|---|
| `path` | texto | pasta no repo de config ou, com `git`, dentro do outro repo; relativa e sem `..` |
| `paths` | lista | so com `git`: varias pastas do mesmo repo, cada uma uma skill |
| `name` | texto | nome da skill; padrao: ultimo segmento do `path`; nao vale com `paths` |
| `git` | texto | outro repo de onde vem a pasta |
| `ref` | texto | obrigatorio com `git`; so vale com `git` |
| `profiles` | lista | como nos MCPs |
| `agents` | lista | como nos MCPs |

- Use `path` ou `paths`, nunca os dois.
- Com `paths`, cada pasta vira uma skill independente com o nome do ultimo segmento; todas compartilham `git`, `ref`, `profiles` e `agents`, e o repo e baixado uma vez so. Uma pasta sem `SKILL.md` e pulada e reportada, sem bloquear as outras.
- Pasta local (sem `git`) sem `SKILL.md` e erro de validacao.
- Nome de skill repetido e erro.
- Uma skill com `git` nao pode ter o nome de um MCP com `source`; use `name`.

## plugins[]

| campo | tipo | obrigatorio | descricao |
|---|---|---|---|
| `name` | texto | sim | nome no catalogo do marketplace (no Antigravity, o `name` do `plugin.json`) |
| `marketplace.git` | texto | sim | URL git, atalho do GitHub ou pasta local com `.claude-plugin` |
| `marketplace.ref` | texto | sim | branch, tag ou SHA completo; a mesma em todas as entradas do mesmo marketplace |
| `profiles` | lista | nao | como nos MCPs |
| `agents` | lista | nao | como nos MCPs |

Nome de plugin repetido e erro. Veja [Plugins](../guias/plugins.md).

## imports

```yaml
imports:
  mcps: [ ... ]
  skills: [ ... ]
  plugins: [ ... ]
```

Cada entrada:

| campo | tipo | obrigatorio | descricao |
|---|---|---|---|
| `git` | texto | sim | repo com um `agsync.yaml` na raiz |
| `ref` | texto | nao | branch, tag ou SHA completo; padrao `main` |
| `profiles` | lista | nao | o import so vale nesses perfis; os itens importados assumem esses perfis |
| `excludes` | lista | nao | nomes daquele tipo, daquela origem, que nao entram; nome inexistente gera aviso |
| `secrets` | lista | nao | so em `imports.mcps`: variaveis que os MCPs importados podem usar como `${VAR}`; nomes com letras, numeros e `_` |

O mesmo `git` com a mesma `ref` repetido no mesmo tipo e erro. Veja [Importar de outros repos](../guias/imports.md).
