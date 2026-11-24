# Segurança

## Quem escreve no repo, executa na sua máquina

`source.build` executa comandos de terceiros. Na prática, quem escreve no repo de config executa código em todas as máquinas que aplicam esse repo. Por isso:

- Proteja a branch seguida pelo agsync com revisão obrigatória de pull request.
- A confirmação do `apply` lista cada build, com repo, commit e comandos, antes de rodar qualquer coisa. Leia antes de confirmar.
- Fixe `ref` por SHA completo quando não quiser seguir uma branch.

## Imports

Um repo em `imports` também vira quem escreve na sua máquina. Por isso:

- MCPs importados só leem os segredos listados em `imports.mcps[].secrets`;
- builds de MCP importado sempre pedem confirmação, mesmo com `silentUpdate` ou `--silent-update`;
- a origem de cada item importado aparece no `diff` e na confirmação;
- `imports` do repo importado não são seguidos.

## Git

- URLs com senha ou token embutido são recusadas; credenciais ficam no credential helper do git.
- Valor de `git` ou `ref` que começa com `-` é recusado, e o agsync chama o `git` com `--end-of-options` e com o transporte `ext::` desligado.
- SHA abreviado é recusado, para que um `ref` fixo não seja ambíguo.

## Segredos

- `secrets.env` e a pasta de backups são acessíveis só pelo usuário (`0600`/`0700` no Linux; ACL protegida com uma única entrada no Windows). O `doctor` confere.
- Valores de `env` e `headers` nunca aparecem na saída do agsync, e o hash do state não depende deles.
- Quando o valor vem do ambiente, o agente que expande variáveis recebe `${VAR}`, não o valor.

## O que nunca acontece

- O `check` nunca aplica nada, e o `hook` nunca acessa a rede.
- O agsync nunca pede senha.
- Backups nunca são copiados de volta automaticamente.

## Binários

Os binários das releases não têm assinatura de código. Os scripts de instalação e o `agsync update` conferem o SHA256 com o `SHA256SUMS` da release antes de instalar. Veja [Microsoft Defender e SmartScreen](../solucao-de-problemas.md#microsoft-defender-e-smartscreen).
