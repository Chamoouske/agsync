# Perfis

Perfis separam o que vale em cada máquina: por exemplo, um MCP interno só no notebook do trabalho.

1. Declare os perfis possíveis no `agsync.yaml`:

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

3. Escolha os perfis de cada máquina no `init`:

   ```sh
   agsync init https://github.com/<org>/agent-config --profile work --profile personal
   ```

Regras:

- Um item com `profiles` só vale nas máquinas com um desses perfis ativos. Um item sem `profiles` vale em todas.
- Citar um perfil que não está em `profiles` é erro de validação.
- Um item que sai do perfil ativo sai dos agentes no próximo `apply`.
- Plugin e import de perfil inativo nem são baixados.

## Trocar os perfis de uma máquina

Os perfis ficam em `profiles` no `machine.toml` (veja [machine.toml](../referencia/machine-toml.md)). Edite o arquivo e rode `agsync apply`, ou rode `agsync init` de novo com os perfis novos.
