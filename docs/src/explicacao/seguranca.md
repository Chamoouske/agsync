# Seguranca

## Quem escreve no repo, executa na sua maquina

`source.build` executa comandos de terceiros. Na pratica, quem escreve no repo de config executa codigo em todas as maquinas que aplicam esse repo. Por isso:

- Proteja a branch seguida pelo agsync com revisao obrigatoria de pull request.
- A confirmacao do `apply` lista cada build, com repo, commit e comandos, antes de rodar qualquer coisa. Leia antes de confirmar.
- Fixe `ref` por SHA completo quando nao quiser seguir uma branch.

## Imports

Um repo em `imports` tambem vira quem escreve na sua maquina. Por isso:

- MCPs importados so leem os segredos listados em `imports.mcps[].secrets`;
- builds de MCP importado sempre pedem confirmacao, mesmo com `silentUpdate` ou `--silent-update`;
- a origem de cada item importado aparece no `diff` e na confirmacao;
- `imports` do repo importado nao sao seguidos.

## Git

- URLs com senha ou token embutido sao recusadas; credenciais ficam no credential helper do git.
- Valor de `git` ou `ref` que comeca com `-` e recusado, e o agsync chama o `git` com `--end-of-options` e com o transporte `ext::` desligado.
- SHA abreviado e recusado, para que um `ref` fixo nao seja ambiguo.

## Segredos

- `secrets.env` e a pasta de backups sao acessiveis so pelo usuario (`0600`/`0700` no Linux; ACL protegida com uma unica entrada no Windows). O `doctor` confere.
- Valores de `env` e `headers` nunca aparecem na saida do agsync, e o hash do state nao depende deles.
- Quando o valor vem do ambiente, o agente que expande variaveis recebe `${VAR}`, nao o valor.

## O que nunca acontece

- O `check` nunca aplica nada, e o `hook` nunca acessa a rede.
- O agsync nunca pede senha.
- Backups nunca sao copiados de volta automaticamente.

## Binarios

Os binarios das releases nao tem assinatura de codigo. Os scripts de instalacao e o `agsync update` conferem o SHA256 com o `SHA256SUMS` da release antes de instalar. Veja [Microsoft Defender e SmartScreen](../solucao-de-problemas.md#microsoft-defender-e-smartscreen).
