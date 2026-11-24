# agsync.yaml

Fica na raiz do repo de config. O agsync valida o arquivo inteiro antes de planejar; campos desconhecidos são erro, e cada erro aponta a linha.

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

| campo | tipo | obrigatório | descrição |
|---|---|---|---|
| `version` | inteiro | sim | sempre `1` |
| `profiles` | lista de nomes | não | todos os perfis que os itens podem citar |
| `silentUpdate` | booleano | não | `true` faz o `agsync init` aplicar sem perguntar, como `--silent-update`; padrão `false` |
| `mcps` | mapa nome -> MCP | não | MCP servers |
| `skills` | lista | não | skills |
| `plugins` | lista | não | plugins de marketplaces |
| `imports` | objeto | não | itens de outros repos de config |

## Regras comuns

- **Nomes** de MCP, skill, plugin e `excludes`: letras, números, `-` e `_`.
- **`profiles`** de um item: cada perfil precisa estar em `profiles` da raiz. Sem `profiles` (ou lista vazia), o item vale em todos os perfis.
- **`agents`** de um item: `claude`, `codex`, `opencode`, `hermes`, `antigravity`. Sem `agents`, vale em todos. Lista vazia é erro.
- **`git`**: URL, atalho do GitHub ou pasta local, conforme o campo. URL com senha ou token embutido é recusada (exceto o usuário de `ssh://git@host`); use o credential helper do git. Valor que começa com `-` é recusado.
- **`ref`**: branch, tag ou SHA completo. SHA abreviado e valor que começa com `-` são recusados. Branch e tag são seguidas: um commit novo aparece no `check`.
- **`${VAR}`**: aceito em `command`, `args`, `env`, `url` e `headers`. Veja [Segredos](../guias/segredos.md).

## mcps.&lt;nome&gt;

| campo | tipo | descrição |
|---|---|---|
| `command` | texto | executável de um MCP stdio |
| `args` | lista de texto | argumentos; só com `command` |
| `env` | mapa | variáveis de ambiente; só com `command` |
| `url` | texto | endereço de um MCP remoto (HTTP) |
| `headers` | mapa | cabeçalhos HTTP; só com `url` |
| `profiles` | lista | perfis em que o item vale |
| `agents` | lista | agentes em que o item vale |
| `source` | objeto | construir o servidor de um repo git |

Defina exatamente um entre `command` e `url`.

### mcps.&lt;nome&gt;.source

| campo | tipo | obrigatório | descrição |
|---|---|---|---|
| `git` | texto | sim | repo do servidor |
| `ref` | texto | sim | branch, tag ou SHA completo |
| `requires` | lista | não | programas que precisam estar no PATH; se faltar algum, o item é pulado |
| `build` | lista | não | comandos rodados no checkout, por `sh -c` no Linux e `cmd /c` no Windows |

`{dir}` em `command`, `args` e `env` vira a pasta do checkout. Veja [MCP construído de um repo git](../guias/mcp-de-fonte-git.md).

## skills[]

Cada item é uma pasta com `SKILL.md`.

| campo | tipo | descrição |
|---|---|---|
| `path` | texto | pasta no repo de config ou, com `git`, dentro do outro repo; relativa e sem `..` |
| `paths` | lista | só com `git`: várias pastas do mesmo repo, cada uma uma skill |
| `name` | texto | nome da skill; padrão: último segmento do `path`; não vale com `paths` |
| `git` | texto | outro repo de onde vem a pasta |
| `ref` | texto | obrigatório com `git`; só vale com `git` |
| `profiles` | lista | como nos MCPs |
| `agents` | lista | como nos MCPs |

- Use `path` ou `paths`, nunca os dois.
- Com `paths`, cada pasta vira uma skill independente com o nome do último segmento; todas compartilham `git`, `ref`, `profiles` e `agents`, e o repo é baixado uma vez só. Uma pasta sem `SKILL.md` é pulada e reportada, sem bloquear as outras.
- Pasta local (sem `git`) sem `SKILL.md` é erro de validação.
- Nome de skill repetido é erro.
- Uma skill com `git` não pode ter o nome de um MCP com `source`; use `name`.

## plugins[]

| campo | tipo | obrigatório | descrição |
|---|---|---|---|
| `name` | texto | sim | nome no catálogo do marketplace (no Antigravity, o `name` do `plugin.json`) |
| `marketplace.git` | texto | sim | URL git, atalho do GitHub ou pasta local com `.claude-plugin` |
| `marketplace.ref` | texto | sim | branch, tag ou SHA completo; a mesma em todas as entradas do mesmo marketplace |
| `profiles` | lista | não | como nos MCPs |
| `agents` | lista | não | como nos MCPs |

Nome de plugin repetido é erro. Veja [Plugins](../guias/plugins.md).

## imports

```yaml
imports:
  mcps: [ ... ]
  skills: [ ... ]
  plugins: [ ... ]
```

Cada entrada:

| campo | tipo | obrigatório | descrição |
|---|---|---|---|
| `git` | texto | sim | repo com um `agsync.yaml` na raiz |
| `ref` | texto | não | branch, tag ou SHA completo; padrão `main` |
| `profiles` | lista | não | o import só vale nesses perfis; os itens importados assumem esses perfis |
| `excludes` | lista | não | nomes daquele tipo, daquela origem, que não entram; nome inexistente gera aviso |
| `secrets` | lista | não | só em `imports.mcps`: variáveis que os MCPs importados podem usar como `${VAR}`; nomes com letras, números e `_` |

O mesmo `git` com a mesma `ref` repetido no mesmo tipo é erro. Veja [Importar de outros repos](../guias/imports.md).
