#!/bin/sh
# Copies the Tessera entry and samples into a checkout of github-linguist/linguist.
#
#   linguist/prepare-pr.sh <linguist-checkout> [<Tessera-website checkout>]
#
# The samples are real programs, not "hello world" (Linguist refuses those), and
# cover the main kinds of Tessera code: a command-line program, a standard module,
# a graphical program and, from the website repository, a pure module of a web
# application. All are MIT licensed.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
linguist=${1:?usage: prepare-pr.sh <linguist-checkout> [<Tessera-website checkout>]}
website=${2:-$here/../Tessera-website}

[ -f "$linguist/lib/linguist/languages.yml" ] || { echo "error: $linguist is not a linguist checkout" >&2; exit 1; }
if grep -q '^Tessera:' "$linguist/lib/linguist/languages.yml"; then
echo "error: languages.yml already has a Tessera entry" >&2
exit 1
fi

samples="$linguist/samples/Tessera"
mkdir -p "$samples"
cp "$here/examples/valid/calculator.tes" "$samples/calculator.tes"
cp "$here/std/config.tes" "$samples/config.tes"
cp "$here/examples/tictactoe/tictactoe.tes" "$samples/tictactoe.tes"
if [ -f "$website/site/totp.tes" ]; then
cp "$website/site/totp.tes" "$samples/totp.tes"
else
echo "warning: $website/site/totp.tes not found; three samples copied" >&2
fi

echo "Samples copied to $samples."
echo "Now add the entry from linguist/languages.yml to lib/linguist/languages.yml"
echo "(between \"Terraform Template\" and \"Texinfo\"), then run script/update-ids."
