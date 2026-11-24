# Plugins

Um plugin e publicado em um marketplace git e instalado pelo CLI do agente.

```yaml
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

- `name` e o nome declarado no catalogo do marketplace.
- `marketplace.git` e `marketplace.ref` sao obrigatorios. Um marketplace pode ter varias entradas, sempre com a mesma `ref`.
- Credenciais ficam no credential helper do git. URL com senha ou token embutido e recusada.

## Suporte por agente

| agente | suporte | como |
|---|---|---|
| Claude Code | sim | `claude plugin marketplace add` e `plugin install/update/uninstall` |
| OpenCode | sim | `opencode plugin add/remove`, gravando o array `plugins` |
| Codex | nao | o CLI nao instala plugin de forma nao interativa; o item e pulado com o motivo no `status` |
| Hermes | nao | idem |
| Antigravity CLI | sim | `agy plugin install <pasta>` a partir de um checkout do marketplace na `ref`, e `agy plugin uninstall` |

## Ciclo de vida

- O `check` acusa plugin adicionado, com `ref` nova e removido do `agsync.yaml`.
- O `diff` e a confirmacao do `apply` listam as mudancas de plugin por agente.
- Plugin que sai do `agsync.yaml` e desinstalado no `apply` seguinte.
- Plugin de perfil inativo nao e baixado nem instalado.
- O `diff` avisa quando um plugin tem o mesmo nome de um MCP ou skill do mesmo agente: os dois podem aparecer lado a lado no agente.
- O `uninstall` desinstala os plugins do `state.json`; os dados do plugin ficam no disco (`--keep-data` no Claude).
- Plugin instalado a mao so e alterado com `apply --adopt`. Veja [Assumir instalacoes manuais](adotar.md).

## Limitacoes

- Marketplace em pasta local: o Claude le a pasta direto e nao aceita `ref` nela. A `ref` continua na identidade do plugin, entao troca-la ainda gera um `update`.
- Trocar a `ref` de um marketplace remoto no Claude: o Claude nao troca a `ref` no lugar, e remover o marketplace desinstala todos os plugins dele. O agsync remove e readiciona o marketplace e reinstala o que gerencia, cada plugin no seu escopo. Se a `ref` nova nao puder ser adicionada, ele readiciona a antiga com os plugins e diz o que nao voltou. Se houver plugin fora do agsync no mesmo marketplace, ele recusa e pede a troca manual.
- O marketplace em si fica registrado no Claude; o agsync nao remove marketplaces no `uninstall`.
- Antigravity nao tem marketplace: `marketplace.git` e o repo (ou a pasta) do proprio plugin, e `name` precisa ser o `name` do `plugin.json`.
- O Antigravity nao troca a `ref` no lugar: o agsync desinstala e instala de novo. Se a instalacao nova falhar, o plugin fica fora ate o proximo `apply`.
- O `agy` nao aceita `ref`: o agsync baixa o marketplace na `ref` e instala da pasta baixada. O que ele instalou fica marcado em `~/.gemini/config/plugins/<nome>/.agsync-source.json`; um plugin com o mesmo nome sem essa marca conta como instalacao manual.
