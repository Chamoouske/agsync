# Planejamento e state

## O state

O `state.json` guarda, por agente e por tipo (MCPs, skills, plugins), cada entrada que o agsync escreveu:

- `hash`: hash do conteudo escrito, calculado sem os valores dos segredos;
- `sha`: o commit da fonte git, para itens construidos ou copiados de uma;
- `mode`: como a skill foi ligada (`symlink`, `junction` ou `copy`);
- `plugin`: o id que o agente usa para desinstalar o plugin.

Tambem guarda o commit aplicado e o anterior, que o `rollback` usa.

## As tres fontes de um plano

Para cada agente e cada tipo, o plano compara tres mapas por nome:

- **desejado**: o que o `agsync.yaml` (com imports, perfis e `agents`) quer para esse agente;
- **rastreado**: o que esta no `state.json`;
- **atual**: o que o agente tem hoje, lido do arquivo ou do CLI dele.

## Acoes

| acao | quando | escreve? |
|---|---|---|
| adicionar | desejado, nao rastreado, nome livre no agente | sim |
| atualizar | rastreado, intacto, conteudo desejado mudou | sim |
| desvio manual, sobrescrever com backup | rastreado, mas mudado ou apagado a mao | sim |
| assumir instalacao manual | nao rastreado, nome ocupado por entrada manual, com `--adopt` | sim |
| conflito | nao rastreado, nome ocupado por entrada manual, sem `--adopt` | nao |
| remover | rastreado, nao mais desejado, ainda presente | sim |
| esquecer | rastreado, nao mais desejado, ja removido fora do agsync | nao, so tira do state |
| sem mudanca | ja como desejado | nao |

Nomes que so existem no agente, sem estar no state nem no `agsync.yaml`, sao entradas manuais e nunca aparecem no plano.

Cada tipo e planejado so contra as entradas do mesmo tipo do agente: uma skill `x` nunca conflita com um MCP `x`.

## Escrita

- Antes de escrever em um agente, o adaptador copia os arquivos de config dele para `<dados>/backups/`.
- Cada item e escrito separadamente. Uma falha num item e mostrada e o item nao entra no state; os outros seguem.
- Arquivos do agsync (state, pendencias, `machine.toml`, `secrets.env`) sao gravados de forma atomica: arquivo temporario na mesma pasta e depois rename.
- O lock em `<dados>/agsync.lock` garante uma execucao por vez escrevendo state e pendencias.
