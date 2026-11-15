# Tessera in GitHub Linguist (draft)

GitHub decides the language of a file with [Linguist](https://github.com/github-linguist/linguist). Linguist does not know Tessera yet, so `.tes` files count as "Other" in the language bar and get no highlighting. A `.gitattributes` override cannot fix this, because `linguist-language=` only accepts languages Linguist already knows. This directory holds everything needed for the pull request that adds Tessera.

| File | Purpose |
|---|---|
| `languages.yml` | The entry for `lib/linguist/languages.yml`. `.tes` and the name Tessera were both still free when this was written (2026-11-15). The color is the blue of the logo. |
| `prepare-pr.sh` | Copies the samples (and checks for an existing entry) into a Linguist checkout: `linguist/prepare-pr.sh ../linguist ../Tessera-website`. |

## When it can be submitted

Linguist's [CONTRIBUTING.md](https://github.com/github-linguist/linguist/blob/main/CONTRIBUTING.md) requires use in the wild: **at least 2,000 `.tes` files indexed by GitHub in the last year, excluding forks**, spread over many different users and repositories (results dominated by one user are filtered out with `-user:...`). It also says that PRs for "very new or hobby languages" are closed. A PR before then will be closed, so watch this search first:

<https://github.com/search?type=code&q=NOT+is%3Afork+path%3A*.tes>

Other formats may also use the `.tes` extension; the reviewers will check that the results really are Tessera, so count only those.

## Steps

1. Fork and clone `github-linguist/linguist`, then run `script/bootstrap` (see its CONTRIBUTING.md; Docker is needed only for grammars).
2. Run `linguist/prepare-pr.sh <linguist-checkout> <Tessera-website checkout>`.
3. Add the entry from `languages.yml` to `lib/linguist/languages.yml` in alphabetical order, then run `script/update-ids` to give it its `language_id`.
4. Run the tests (`bundle exec rake test`).
5. Open the PR with Linguist's template filled in: link the search above, and state that the samples are MIT licensed and come from `dib0/Tessera-language` and `dib0/Tessera-website`.

## Highlighting

The entry uses `tm_scope: none`: Tessera has no TextMate grammar yet, so GitHub would recognize the files without highlighting them. A grammar can be added later in its own repository (with an accepted license) and added with `script/add-grammar`; then `tm_scope` becomes its scope, for example `source.tessera`. The keyword list and token rules in Tessera-website's `wwwroot/assets/js/site.js` highlighter are a starting point.
