# Tesseradoom

A DOOM-style engine written in Tessera, following the DOOM roadmap (milestones M1 to M9). It is the
largest example program in this repository and the test bed for the language features it needs.

## Game data: not included

No WAD file is part of this repository, and none may be committed (decision D5). Put one in the
`WAD/` directory next to this file; everything in it except its `.gitignore` is ignored by git.

The engine is developed and tested with **Freedoom** (`freedoom1.wad`, the episode-based IWAD with
maps `E1M1` to `E4M9`):

1. Download the latest release from <https://freedoom.github.io/download.html>
   (developed against Freedoom 0.13.0).
2. Unpack it and copy `freedoom1.wad` to `WAD/freedoom1.wad`.

Freedoom 0.13.0's `freedoom1.wad` has SHA-256
`7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d`.

Freedoom is distributed under a BSD-style license (see `COPYING.txt` in the Freedoom download).
Other IWADs, such as the shareware `doom1.wad` or a commercial `doom.wad`, should work too, but only
use files you have the right to use, and never commit them.

## License and sources

The engine is MIT licensed, like the rest of this repository (decision D4). It is written from
public descriptions of the WAD format and the game, not from the GPL DOOM source code or GPL ports
such as gldoom. Do not copy or translate code from them into this directory.

## Design decisions

The decisions D1 to D5 that shape this port are recorded in
[`docs/doom-decisions.md`](../../docs/doom-decisions.md): window and input through an extended `ui`
effect, 16.16 fixed-point numbers, at least 35 frames per second at 320 x 200, Freedoom as the game
data, and no bundled WAD files.

## Status

Set up only; no engine code yet. The milestones and their checks are listed in the DOOM roadmap.
Tests that need game data read it from `WAD/` and are skipped when it is missing.
