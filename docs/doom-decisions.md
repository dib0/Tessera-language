# DOOM port decisions (D1 to D5)

Decision record for the DOOM roadmap, Phase 5. Decided by Bas van den Berg on 2026-11-10. The port
lives in [`examples/tesseradoom/`](../examples/tesseradoom/README.md).

## D1. Window and input: extend the `ui` effect

The existing `ui` effect (section 9.4 of `language-design.md`: X11 in safe Rust, no FFI, no
`unsafe`) is extended rather than adding a native host module (roadmap option a) or piping frames
to an external viewer (option b). Implemented in draft 0.18:

- `draw_pixels(x, y, width, colors: List[Int], scale)`: a whole frame in one call, instead of one
  rectangle per pixel. `scale` was added during implementation, so a 320 x 200 frame fills a
  640 x 400 window without the program building a four times larger list every frame.
- `poll_event() -> Option[WindowEvent]`: returns immediately, so a game loop never blocks on input.
- A key-release event (`KeyUp`), so held keys can be tracked.

Sound is not part of D1; it is not needed for any milestone.

Headless milestones (M4 to M6) keep writing frames as PPM images (`std/ppm`).

## D2. Number model: 16.16 fixed point

Game arithmetic uses 16.16 fixed point in `Int` (section 7.6 of `language-design.md`, `std/fixed`),
not `Float`. Fixed point gives the same results on every platform, which demo playback (M9) needs;
`Float` library functions such as `sin` can differ in the last bit between platforms. `Float` may
still be used for things outside the simulation, such as frame-time statistics.

## D3. Performance: at least 35 fps at 320 x 200, measured

The target is at least 35 frames per second at 320 x 200 on the development machine, built with
`--opt 3`. Frame time is measured from M4 on and recorded with each milestone. If the target is
missed, first profile and improve the generated Rust or the runtime; change the data model only
when that is not enough.

Measured (2026-11-10, E1M1, `bench` and `play --uncapped`): about 1,000 frames per second for
rendering alone and 684 in the window at 640 x 400. The first measurement was 44 frames per second
for the starting view; the cause was in the compiler (`set c = f(c)` copied records instead of
moving them, so every drawing call copied the frame buffer), and fixing it there (draft 0.18) made
rendering about 20 times faster without changing the program.

## D4. License: MIT engine, Freedoom game data

The engine stays MIT, like the rest of the repository, and is written from public descriptions of
the WAD format and the game (for example the Unofficial Doom Specs), without copying or translating
GPL code (the DOOM source release, gldoom). The game data is Freedoom, whose BSD-style license is
compatible with that choice.

Consequence to watch: exact demo parity (M9) depends on details of the original game logic, such as
its random number sequence and the order in which things update. Each such detail must come from a
source that is not GPL code. If that is not possible for a detail, M9's check is relaxed for it and
the reason is recorded here.

## D5. Game data: never bundled

No WAD file is committed. Game data lives in `examples/tesseradoom/WAD/`, which git ignores except
for its `.gitignore`. The port's README says where to get Freedoom (`freedoom1.wad`, release 0.13.0,
SHA-256 `7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d`). Development and the
milestone checks use Freedoom; tests that need a WAD are skipped when the file is missing.

Because Freedoom replaces the shareware `doom1.wad` named in the roadmap, the milestone checks run
against `freedoom1.wad` (which has `E1M1`, `PLAYPAL` and `DEMO1` to `DEMO4`).
