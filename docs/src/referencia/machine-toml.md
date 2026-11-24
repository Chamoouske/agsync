# machine.toml

Configuracao desta maquina. O `agsync init` grava o arquivo na pasta de config (veja [Arquivos e pastas](arquivos.md)). Pode ser editado a mao; campos desconhecidos sao erro.

```toml
repo = "https://github.com/<org>/agent-config"
ref = "main"
profiles = ["work", "personal"]
check_interval_minutes = 15
```

| campo | tipo | padrao | descricao |
|---|---|---|---|
| `repo` | texto | obrigatorio | URL git do repo de config |
| `ref` | texto | `main` | branch, tag ou SHA completo seguido |
| `profiles` | lista de texto | `[]` | perfis ativos nesta maquina |
| `check_interval_minutes` | inteiro | `15` | intervalo do `agsync check` periodico; `0` e erro |

- Sem `machine.toml`, os comandos que precisam dele pedem `agsync init <url> --profile P` antes.
- Mudou `check_interval_minutes`? Rode `agsync schedule install` para regravar o agendamento. O `agsync hook` tambem usa o intervalo para decidir quando disparar um `check` em segundo plano.
- Mudou `profiles` ou `ref`? O proximo `status`, `diff` ou `apply` ja usa os valores novos.
