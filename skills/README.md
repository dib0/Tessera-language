# Claude Code skill for Tessera

`tessera/` is a [Claude Code](https://claude.com/claude-code) skill that teaches the model to write, fix and run Tessera programs. `SKILL.md` holds the workflow and the common mistakes; `reference.md` (the harness language reference, `harness/reference/tessera.md`) and `language-design.md` (the specification) are symbolic links into this repository, so the skill always matches the checkout.

## Install

For every project on your machine, link it into your user skills (stays up to date with `git pull`):

```sh
mkdir -p ~/.claude/skills
ln -s "$PWD/skills/tessera" ~/.claude/skills/tessera
```

For one project only, use that project's `.claude/skills/` instead of `~/.claude/skills/`.

To copy it somewhere without this checkout, resolve the links (`-L`), or the copy contains broken links:

```sh
cp -rL skills/tessera ~/.claude/skills/tessera
```

A copy does not follow later language changes; copy it again after updating the repository.

The skill expects the compiler `tessera1` on `PATH` or in a checkout's `bin/` (built with `./build.sh`).
