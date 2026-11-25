# Plugins

Um plugin é publicado em um marketplace git e instalado pelo CLI do agente.

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

- `name` é o nome declarado no catálogo do marketplace.
- `marketplace.git` e `marketplace.ref` são obrigatórios. Um marketplace pode ter várias entradas, sempre com a mesma `ref`.
- Credenciais ficam no credential helper do git. URL com senha ou token embutido é recusada.

## Suporte por agente

| agente | suporte | como |
|---|---|---|
| Claude Code | sim | `claude plugin marketplace add` e `plugin install/update/uninstall` |
| OpenCode | sim | `opencode plugin add/remove`, gravando o array `plugins` |
| Codex | sim | `codex plugin marketplace add <origem> --ref <ref>` e `codex plugin add/remove <plugin>@<marketplace>` |
| Hermes | não | o CLI não instala plugin de forma não interativa; o item é pulado com o motivo no `status` |
| Antigravity CLI | sim | `agy plugin install <pasta>` a partir de um checkout do marketplace na `ref`, e `agy plugin uninstall` |

## Ciclo de vida

- O `check` acusa plugin adicionado, com `ref` nova e removido do `agsync.yaml`.
- O `diff` e a confirmação do `apply` listam as mudanças de plugin por agente.
- Plugin que sai do `agsync.yaml` é desinstalado no `apply` seguinte.
- Plugin de perfil inativo não é baixado nem instalado.
- O `diff` avisa quando um plugin tem o mesmo nome de um MCP ou skill do mesmo agente: os dois podem aparecer lado a lado no agente.
- O `uninstall` desinstala os plugins do `state.json`; os dados do plugin ficam no disco (`--keep-data` no Claude).
- Falha de um comando do agente é mostrada com o comando, o código de saída e a saída dele. O plugin não entra no `state.json` e volta no próximo `apply`.
- Plugin instalado à mão só é alterado com `apply --adopt`. Veja [Assumir instalações manuais](adotar.md).

## Limitações

- Marketplace em pasta local: o Claude lê a pasta direto e não aceita `ref` nela. A `ref` continua na identidade do plugin, então trocá-la ainda gera um `update`.
- Trocar a `ref` de um marketplace remoto no Claude: o Claude não troca a `ref` no lugar, e remover o marketplace desinstala todos os plugins dele. O agsync remove e readiciona o marketplace e reinstala o que gerencia, cada plugin no seu escopo. Se a `ref` nova não puder ser adicionada, ele readiciona a antiga com os plugins e diz o que não voltou. Se houver plugin fora do agsync no mesmo marketplace, ele recusa e pede a troca manual.
- O marketplace em si fica registrado no agente (Claude e Codex); o agsync não remove marketplaces no `uninstall`.
- Trocar a `ref` no Codex: o Codex guarda uma `ref` por marketplace e recusa readicioná-lo com outra. O agsync remove o marketplace, adiciona na `ref` nova e reinstala cada plugin que gerencia nele. Os plugins instalados continuam instalados durante a troca. Se a `ref` nova falhar, ele readiciona a antiga e avisa. Se houver plugin fora do agsync no mesmo marketplace, ele recusa e pede a troca manual.
- O Codex lê o catálogo em `.agents/plugins/marketplace.json` ou `.claude-plugin/marketplace.json`, e o plugin em `.codex-plugin/plugin.json` ou `.claude-plugin/plugin.json`.
- Antigravity não tem marketplace: `marketplace.git` é o repo (ou a pasta) do próprio plugin, e `name` precisa ser o `name` do `plugin.json`.
- O Antigravity não troca a `ref` no lugar: o agsync desinstala e instala de novo. Se a instalação nova falhar, o plugin fica fora até o próximo `apply`.
- O `agy` não aceita `ref`: o agsync baixa o marketplace na `ref` e instala da pasta baixada. O que ele instalou fica marcado em `~/.gemini/config/plugins/<nome>/.agsync-source.json`; um plugin com o mesmo nome sem essa marca conta como instalação manual.
