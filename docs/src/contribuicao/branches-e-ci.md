# Branches e CI

## Fluxo de branches

O repo publica no máximo uma release por dia, fora hotfixes:

- **Mudança normal**: crie a branch a partir de `stage` e abra o PR para `stage` (a branch padrão do repo).
- **Promoção diária**: o workflow `promote` roda todo dia às 00:00 UTC (21:00 em Brasília) e também pode ser disparado na mão (Actions > promote > Run workflow). Se `stage` tem mudanças que `master` não tem, ele abre o PR `stage` -> `master` e liga o auto-merge com merge commit. Se os checks obrigatórios do commit de `stage` já passaram, o PR nasce pronto para merge e o GitHub recusa o auto-merge ("clean status"); nesse caso o workflow faz o merge direto, preso ao commit que foi checado (`--match-head-commit`). Se nesse meio tempo o CI do PR entrou na fila, o ruleset bloqueia o merge direto, e o workflow espera alguns segundos e liga o auto-merge de novo, até 6 tentativas (`scripts/merge-pr.sh`). Qualquer outra falha faz o job falhar. O merge em `master` dispara a release. Sem mudanças, não faz nada. O agendamento do GitHub pode atrasar alguns minutos.
- **Hotfix**: crie a branch a partir de `master` e abra o PR direto para `master`. O merge dispara a release na hora.
- **Volta para `stage`**: a cada push em `master` (promoção, hotfix ou PR de manifests), o workflow `sync-stage` abre o PR `master` -> `stage` com auto-merge, para `stage` sempre conter `master`. Como no `promote`, se o PR já nasce pronto para merge, o workflow faz o merge direto. Se esse PR tiver conflito, o job falha e o PR fica aberto: resolva numa branch a partir de `stage` e faça o merge na mão.

Os PRs `stage` -> `master` e `master` -> `stage` são sempre mergeados com merge commit, nunca squash ou rebase: o squash juntaria os commits num título só e o `scripts/next-version.sh` perderia os `feat`/`fix`, e os dois deixariam `stage` e `master` divergentes.

## CI obrigatória

Toda mudança entra em `stage` ou `master` por PR. Os rulesets `stage` ([`.github/rulesets/stage.json`](https://github.com/Chamoouske/agsync/blob/stage/.github/rulesets/stage.json)) e `master` ([`.github/rulesets/master.json`](https://github.com/Chamoouske/agsync/blob/stage/.github/rulesets/master.json)) exigem:

- os checks `check (ubuntu-latest)` e `check (windows-latest)` do workflow `ci` (`cargo fmt --check`, `cargo clippy -D warnings` e `cargo test`), vindos do GitHub Actions;
- nenhum push direto, force push ou remoção da branch.

Só o `master` exige também a branch do PR atualizada com `master` antes do merge. O `stage` não exige, senão o PR `master` -> `stage` nunca poderia ser mergeado.

Enquanto um check obrigatório estiver pendente, rodando ou falho, o botão de merge fica bloqueado. Não há exceção por autor ou aprovador, nem para admins: os rulesets não têm bypass. Se um check falhar, corrija na branch e faça push; o CI roda de novo. Num PR para `master`, se `master` andar, atualize a branch ("Update branch") e espere o CI.

Admins do repo podem editar ou desativar os rulesets em Settings > Rules. Para reaplicar a versão do repo:

```bash
gh api -X PUT repos/Chamoouske/agsync/rulesets/27269112 --input .github/rulesets/master.json
gh api -X PUT repos/Chamoouske/agsync/rulesets/27474564 --input .github/rulesets/stage.json
```

## Workflows

| workflow | quando | o que faz |
|---|---|---|
| `ci` | push, PR e chamado pelo `release` | job `check` (fmt, clippy, testes) em Windows e Linux; job `manifests` (actionlint, testes dos scripts, manifests contra o binário publicado, schemas) |
| `conventional-commits` | PR (aberto, editado, push ou reaberto) | check `conventional commits`: título do PR e commits (fora merges) em Conventional Commits |
| `docs` | PR e push em `stage` ou `master` que mexem em `docs/`, no `CHANGELOG.md` ou no próprio workflow | gera a ajuda da CLI e confere que está em dia, roda `mdbook build` e confere os links internos; em push em `master`, publica no GitHub Pages |
| `promote` | todo dia às 00:00 UTC e manual | PR `stage` -> `master` |
| `sync-stage` | push em `master` e manual | PR `master` -> `stage` |
| `release` | push em `master` fora de docs e packaging, e manual | build, release e manifests; veja [Releases](releases.md) |

## Dependabot

O [`.github/dependabot.yml`](https://github.com/Chamoouske/agsync/blob/stage/.github/dependabot.yml) abre, toda segunda-feira às 06:00 (horário de Brasília), PRs para `stage` com as atualizações de crates do `Cargo.toml` e das actions dos workflows. As atualizações minor e patch vêm num PR só por ecossistema; cada major vem num PR próprio. No máximo 5 PRs abertos por ecossistema.

Os títulos seguem Conventional Commits: `chore(deps): ...` para crates (seção Manutenção do changelog) e `ci(deps): ...` para actions (seção CI). Nenhum dos dois sobe minor ou major: na promoção, sozinhos, geram uma release patch. Os checks obrigatórios rodam nesses PRs como em qualquer outro, porque nenhum deles usa secrets.
