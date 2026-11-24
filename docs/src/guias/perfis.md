# Perfis

Perfis separam o que vale em cada maquina: por exemplo, um MCP interno so no notebook do trabalho.

1. Declare os perfis possiveis no `agsync.yaml`:

   ```yaml
   profiles: [work, personal]
   ```

2. Restrinja os itens:

   ```yaml
   mcps:
     internal-tool:
       profiles: [work]
       url: https://mcp.internal.example/mcp
   ```

3. Escolha os perfis de cada maquina no `init`:

   ```sh
   agsync init https://github.com/<org>/agent-config --profile work --profile personal
   ```

Regras:

- Um item com `profiles` so vale nas maquinas com um desses perfis ativos. Um item sem `profiles` vale em todas.
- Citar um perfil que nao esta em `profiles` e erro de validacao.
- Um item que sai do perfil ativo sai dos agentes no proximo `apply`.
- Plugin e import de perfil inativo nem sao baixados.

## Trocar os perfis de uma maquina

Os perfis ficam em `profiles` no `machine.toml` (veja [machine.toml](../referencia/machine-toml.md)). Edite o arquivo e rode `agsync apply`, ou rode `agsync init` de novo com os perfis novos.
