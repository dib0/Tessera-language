# Making a release

A release is a tag, a GitHub release, and two archive copies. The archive copies
are the point: they give the work a permanent home and a citable identifier that
does not depend on GitHub, and they are what a reader, an index or a dataset can
point at years later.

Releases follow the language draft, so draft 0.21 is `v0.21`. `CITATION.cff`
carries the same number and must be updated in the same commit.

## 1. Check the tree

```sh
./build.sh                                   # fixpoint, and regenerates web/llms*.txt
cd compiler && cargo test --release --no-fail-fast
bin/tessera1 format --check $(git ls-files '*.tes')
harness/.venv/bin/python harness/harness.py dry-run --variants baseline
git status --porcelain                       # must be empty
```

`version:` and `date-released:` in `CITATION.cff` must match the draft number at
the top of `language-design.md` and today's date.

## 2. Tag

```sh
git tag -a v0.21 -m "Tessera draft 0.21"
git push origin v0.21
```

An annotated tag (`-a`), not a lightweight one: it carries a date and an author,
which is what the archives record.

## 3. GitHub release

Create a release from the tag. For the notes, take the draft's rows from section
3 of `language-design.md` ("Changes from earlier drafts") and the test and status
figures from `docs/status.md`. Say plainly what is still an early proof of
concept; the project's credibility rests on the measurements being honest about
their limits.

## 4. Archive

**Zenodo** mints a DOI per release. Connect the repository once at
<https://zenodo.org/account/settings/github/>, after which every GitHub release
is archived and gets its own DOI, plus a concept DOI for the project as a whole.
Add the concept DOI to `CITATION.cff` as `doi:` and to the README badge row.

**Software Heritage** archives the whole repository, independently of GitHub, and
gives every file a permanent identifier. Submit the clone URL once at
<https://archive.softwareheritage.org/save/>; it re-crawls afterwards, but
submitting again after a release makes the new revision appear sooner.

Both are free, take a few minutes, and only have to be set up once.
