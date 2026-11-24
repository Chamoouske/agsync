# Planejamento e state

## O state

O `state.json` guarda, por agente e por tipo (MCPs, skills, plugins), cada entrada que o agsync escreveu:

- `hash`: hash do conteúdo escrito, calculado sem os valores dos segredos;
- `sha`: o commit da fonte git, para itens construídos ou copiados de uma;
- `mode`: como a skill foi ligada (`symlink`, `junction` ou `copy`);
- `plugin`: o id que o agente usa para desinstalar o plugin.

Também guarda o commit aplicado e o anterior, que o `rollback` usa.

## As três fontes de um plano

Para cada agente e cada tipo, o plano compara três mapas por nome:

- **desejado**: o que o `agsync.yaml` (com imports, perfis e `agents`) quer para esse agente;
- **rastreado**: o que está no `state.json`;
- **atual**: o que o agente tem hoje, lido do arquivo ou do CLI dele.

## Ações

| ação | quando | escreve? |
|---|---|---|
| adicionar | desejado, não rastreado, nome livre no agente | sim |
| atualizar | rastreado, intacto, conteúdo desejado mudou | sim |
| desvio manual, sobrescrever com backup | rastreado, mas mudado ou apagado à mão | sim |
| assumir instalação manual | não rastreado, nome ocupado por entrada manual, com `--adopt` | sim |
| conflito | não rastreado, nome ocupado por entrada manual, sem `--adopt` | não |
| remover | rastreado, não mais desejado, ainda presente | sim |
| esquecer | rastreado, não mais desejado, já removido fora do agsync | não, só tira do state |
| sem mudança | já como desejado | não |

Nomes que só existem no agente, sem estar no state nem no `agsync.yaml`, são entradas manuais e nunca aparecem no plano.

Cada tipo é planejado só contra as entradas do mesmo tipo do agente: uma skill `x` nunca conflita com um MCP `x`.

## Escrita

- Antes de escrever em um agente, o adaptador copia os arquivos de config dele para `<dados>/backups/`.
- Cada item e escrito separadamente. Uma falha num item é mostrada e o item não entra no state; os outros seguem.
- Arquivos do agsync (state, pendências, `machine.toml`, `secrets.env`) são gravados de forma atômica: arquivo temporário na mesma pasta e depois rename.
- O lock em `<dados>/agsync.lock` garante uma execução por vez escrevendo state e pendências.
