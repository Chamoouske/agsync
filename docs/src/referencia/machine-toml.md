# machine.toml

Configuração desta máquina. O `agsync init` grava o arquivo na pasta de config (veja [Arquivos e pastas](arquivos.md)). Pode ser editado à mão; campos desconhecidos são erro.

```toml
repo = "https://github.com/<org>/agent-config"
ref = "main"
profiles = ["work", "personal"]
check_interval_minutes = 15
```

| campo | tipo | padrão | descrição |
|---|---|---|---|
| `repo` | texto | obrigatório | URL git do repo de config |
| `ref` | texto | `main` | branch, tag ou SHA completo seguido |
| `profiles` | lista de texto | `[]` | perfis ativos nesta máquina |
| `check_interval_minutes` | inteiro | `15` | intervalo do `agsync check` periódico; `0` é erro |

- Sem `machine.toml`, os comandos que precisam dele pedem `agsync init <url> --profile P` antes.
- Mudou `check_interval_minutes`? Rode `agsync schedule install` para regravar o agendamento. O `agsync hook` também usa o intervalo para decidir quando disparar um `check` em segundo plano.
- Mudou `profiles` ou `ref`? O próximo `status`, `diff` ou `apply` já usa os valores novos.
