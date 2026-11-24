# Usar no WSL

O agsync no WSL e uma instalacao Linux separada. Ele cuida das configs do home Linux; o Windows precisa da sua propria instalacao.

1. No Windows, instale e rode `agsync init` normalmente.
2. Dentro da distro WSL, instale pelo script de Linux e rode `agsync init` de novo, com os perfis que fizerem sentido ali.

Comportamento dentro do WSL:

- Programas do Windows vistos pelo interop (em `/mnt/<letra>/`, ou na `root` de `[automount]` do `/etc/wsl.conf`) sao ignorados como agentes, `requires` e `git`. O `status` e o `doctor` dizem quando um agente so existe no Windows.
- Notificacoes ficam so no terminal, pelo `agsync hook` e pelo `agsync status`, mesmo com o `DISPLAY` do WSLg.
- Sem systemd e sem cron, o `agsync hook` dispara o `check` em segundo plano quando o shell abre e o ultimo `check` com sucesso tem mais que o dobro do intervalo.
- O PATH gravado no agendamento ignora as pastas do Windows.
