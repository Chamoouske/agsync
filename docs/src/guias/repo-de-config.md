# Montar o repo de config

O repo de config e um repo git comum com um `agsync.yaml` na raiz. Todas as maquinas que fazem `agsync init` com ele passam a segui-lo.

## 1. Crie o repo e o agsync.yaml

```text
agent-config/
  agsync.yaml
  skills/
    grug-brain-code/
      SKILL.md
```

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

skills:
  - path: skills/grug-brain-code
```

- `version` e sempre `1`.
- `profiles` lista todos os perfis que os itens podem citar.
- Cada MCP tem `command` (stdio) ou `url` (HTTP), nunca os dois.
- Cada skill e uma pasta com `SKILL.md`.

A lista completa de campos esta em [agsync.yaml](../referencia/agsync-yaml.md).

## 2. Skills de outro repo

Uma skill pode vir de outro repo git, fixada em uma `ref`:

```yaml
skills:
  - git: https://github.com/mattpocock/skills
    ref: 4ae2317ddb22b046e26455e4f9c9b3aaaf62212d
    paths:
      - skills/engineering/tdd
      - skills/engineering/pr
  - git: https://github.com/<org>/more-skills
    ref: main
    path: review
    name: team-review
```

- Com `paths`, cada pasta vira uma skill com o nome do ultimo segmento. O repo e baixado uma vez so.
- Com `path`, `name` troca o nome da skill.
- Uma `ref` de branch ou tag e seguida: um commit novo nela tambem aparece no `check`. Use o SHA completo para fixar.

## 3. Restrinja por agente ou perfil

```yaml
mcps:
  internal:
    url: https://mcp.internal.example/mcp
    profiles: [work]
    agents: [claude, codex]
```

Sem `profiles`, o item vale em todos os perfis. Sem `agents`, em todos os agentes. Veja [Perfis](perfis.md).

## 4. Proteja a branch

Quem escreve no repo de config decide o que roda nas maquinas que o seguem (veja [Seguranca](../explicacao/seguranca.md)). Proteja a branch seguida com revisao obrigatoria de pull request.

## 5. Valide

O agsync valida o `agsync.yaml` inteiro antes de planejar. Um erro aponta a linha:

```text
erro: agsync.yaml linha 12: mcp 'context7': defina exatamente um entre command e url
```

Para conferir sem aplicar, rode `agsync diff` numa maquina que ja fez `init`. Um `agsync.yaml` invalido no remoto tambem aparece no `check` como pendencia, e nada e aplicado.
