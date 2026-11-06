Snapshots of the stage 1 compiler source (without the generated runtime text), kept so that
token measurements can be repeated: `compiler-0.7` is the source before the draft 0.8 rewrite,
`compiler-0.8-shorthand-only` after the `.Variant` rewrite but before interpolation. The current
source is `stage1/`. Compare with:

    harness/.venv/bin/python harness/harness.py tokens --corpora compiler \
        --compare harness/corpora/compiler-0.7 harness/corpora/compiler-0.8-shorthand-only stage1
