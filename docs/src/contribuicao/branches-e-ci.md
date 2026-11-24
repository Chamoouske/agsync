# Branches e CI

## Fluxo de branches

O repo publica no maximo uma release por dia, fora hotfixes:

- **Mudanca normal**: crie a branch a partir de `stage` e abra o PR para `stage` (a branch padrao do repo).
- **Promocao diaria**: o workflow `promote` roda todo dia as 00:00 UTC (21:00 em Brasilia) e tambem pode ser disparado na mao (Actions > promote > Run workflow). Se `stage` tem mudancas que `master` nao tem, ele abre o PR `stage` -> `master` e liga o auto-merge com merge commit. Se os checks obrigatorios do commit de `stage` ja passaram, o PR nasce pronto para merge e o GitHub recusa o auto-merge ("clean status"); nesse caso o workflow faz o merge direto, preso ao commit que foi checado (`--match-head-commit`). Se nesse meio tempo o CI do PR entrou na fila, o ruleset bloqueia o merge direto, e o workflow espera alguns segundos e liga o auto-merge de novo, ate 6 tentativas (`scripts/merge-pr.sh`). Qualquer outra falha faz o job falhar. O merge em `master` dispara a release. Sem mudancas, nao faz nada. O agendamento do GitHub pode atrasar alguns minutos.
- **Hotfix**: crie a branch a partir de `master` e abra o PR direto para `master`. O merge dispara a release na hora.
- **Volta para `stage`**: a cada push em `master` (promocao, hotfix ou PR de manifests), o workflow `sync-stage` abre o PR `master` -> `stage` com auto-merge, para `stage` sempre conter `master`. Como no `promote`, se o PR ja nasce pronto para merge, o workflow faz o merge direto. Se esse PR tiver conflito, o job falha e o PR fica aberto: resolva numa branch a partir de `stage` e faca o merge na mao.

Os PRs `stage` -> `master` e `master` -> `stage` sao sempre mergeados com merge commit, nunca squash ou rebase: o squash juntaria os commits num titulo so e o `scripts/next-version.sh` perderia os `feat`/`fix`, e os dois deixariam `stage` e `master` divergentes.

## CI obrigatoria

Toda mudanca entra em `stage` ou `master` por PR. Os rulesets `stage` ([`.github/rulesets/stage.json`](https://github.com/Chamoouske/agsync/blob/stage/.github/rulesets/stage.json)) e `master` ([`.github/rulesets/master.json`](https://github.com/Chamoouske/agsync/blob/stage/.github/rulesets/master.json)) exigem:

- os checks `check (ubuntu-latest)` e `check (windows-latest)` do workflow `ci` (`cargo fmt --check`, `cargo clippy -D warnings` e `cargo test`), vindos do GitHub Actions;
- nenhum push direto, force push ou remocao da branch.

So o `master` exige tambem a branch do PR atualizada com `master` antes do merge. O `stage` nao exige, senao o PR `master` -> `stage` nunca poderia ser mergeado.

Enquanto um check obrigatorio estiver pendente, rodando ou falho, o botao de merge fica bloqueado. Nao ha excecao por autor ou aprovador, nem para admins: os rulesets nao tem bypass. Se um check falhar, corrija na branch e faca push; o CI roda de novo. Num PR para `master`, se `master` andar, atualize a branch ("Update branch") e espere o CI.

Admins do repo podem editar ou desativar os rulesets em Settings > Rules. Para reaplicar a versao do repo:

```bash
gh api -X PUT repos/Chamoouske/agsync/rulesets/27269112 --input .github/rulesets/master.json
gh api -X PUT repos/Chamoouske/agsync/rulesets/27474564 --input .github/rulesets/stage.json
```

## Workflows

| workflow | quando | o que faz |
|---|---|---|
| `ci` | push, PR e chamado pelo `release` | job `check` (fmt, clippy, testes) em Windows e Linux; job `manifests` (actionlint, testes dos scripts, manifests contra o binario publicado, schemas) |
| `docs` | PR e push em `stage` ou `master` que mexem em `docs/`, no `CHANGELOG.md` ou no proprio workflow | gera a ajuda da CLI e confere que esta em dia, roda `mdbook build` e confere os links internos; em push em `master`, publica no GitHub Pages |
| `promote` | todo dia as 00:00 UTC e manual | PR `stage` -> `master` |
| `sync-stage` | push em `master` e manual | PR `master` -> `stage` |
| `release` | push em `master` fora de docs e packaging, e manual | build, release e manifests; veja [Releases](releases.md) |
