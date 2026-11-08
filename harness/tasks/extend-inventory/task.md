The program below keeps an inventory. Each command-line argument is a command: `add:ITEM:QTY` adds QTY units of ITEM (new items are appended), `remove:ITEM:QTY` removes units. At the end it prints every item with a non-zero quantity as `ITEM QTY`, in order of first addition. Extend it:
1. Make `remove` fail when an item is unknown or has fewer units than requested: print `error: cannot remove QTY ITEM` to standard error and exit with code 1. Use a `Result` returned from the function that applies a command.
2. Add a command `clear:ITEM` that sets the quantity of a known item to 0 (unknown items are ignored).
3. Any other command prints `error: unknown command ARG` (with the whole argument) to standard error and exits with code 1.
