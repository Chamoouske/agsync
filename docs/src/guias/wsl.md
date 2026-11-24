# Usar no WSL

O agsync no WSL é uma instalação Linux separada. Ele cuida das configs do home Linux; o Windows precisa da sua própria instalação.

1. No Windows, instale e rode `agsync init` normalmente.
2. Dentro da distro WSL, instale pelo script de Linux e rode `agsync init` de novo, com os perfis que fizerem sentido ali.

Comportamento dentro do WSL:

- Programas do Windows vistos pelo interop (em `/mnt/<letra>/`, ou na `root` de `[automount]` do `/etc/wsl.conf`) são ignorados como agentes, `requires` e `git`. O `status` e o `doctor` dizem quando um agente só existe no Windows.
- Notificações ficam só no terminal, pelo `agsync hook` e pelo `agsync status`, mesmo com o `DISPLAY` do WSLg.
- Sem systemd e sem cron, o `agsync hook` dispara o `check` em segundo plano quando o shell abre e o último `check` com sucesso tem mais que o dobro do intervalo.
- O PATH gravado no agendamento ignora as pastas do Windows.
