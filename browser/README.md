# Tessera in the browser

`tessera.js` runs Tessera programs in a web page, the way a page runs a WebAssembly module: a model generates a program, as Tessera text or as its JSON syntax tree, and the page checks and runs it. No server, no Rust toolchain. The specification is section 21 of `language-design.md`.

- `tessera.wasm`: the stage 1 compiler built as WebAssembly. It is generated, not committed: `./build.sh` builds it when the target is installed (`rustup target add wasm32-unknown-unknown`), or by hand with `bin/tessera1 build stage1/main.tes --wasm -o browser/tessera.wasm`. About 4.5 MB, 1 MB compressed.
- `tessera.js`: the JavaScript host, an ES module with no dependencies.
- `index.html`: a demo page. Serve the repository root (`python3 -m http.server`) and open `http://localhost:8000/browser/`. Browsers do not load WebAssembly from `file://`.
- `test/run.sh`: runs `test/test.html` in headless Chrome. The test suite runs it as `browser_library_runs_programs`.

## Use

```js
import { Tessera } from "./tessera.js";

const tessera = await Tessera.load();   // fetches tessera.wasm next to tessera.js; or pass a URL, bytes or a Module

const result = tessera.run(`fn main() -> Unit uses io {
print("hello")
}
`);
// { exitCode: 0, stdout: "hello\n", stderr: "", diagnostics: [] }
```

`run(program, options)` interprets `main` (`tessera1 interpret`). `program` is Tessera text, or a syntax tree, given either as an object or as JSON text. Options:

| Option | Meaning |
|---|---|
| `stdin` | Standard input (text or `Uint8Array`), for `read_line` and `read_input` |
| `args` | The program's `args()` |
| `modules` | Further source files by path, for example `{"geometry/shapes.tes": "..."}` for `import geometry/shapes`. `std/...` is built in |
| `env` | Environment variables for `environment_variable`. `TESSERA_INTERPRET_DEPTH` sets the limit on nested calls (default 700) |
| `onStdout`, `onStderr` | Receive output as it is written |

If the program does not compile, `exitCode` is 1 and `diagnostics` holds the errors in the JSON form of `tessera1 check --json`: code, message, position, `help`, `expected`/`actual`, and `fixes` that a model can apply (spec section 20.4). A runtime error gives exit code 70 and `runtime error Rxxx: ...` on `stderr`, with the program's location.

The program may use the effects `io`, `env`, `time`, `random` and `log`. Any other effect in `main` is refused with E0707 before anything runs.

Other calls, each running the compiler's command line on files kept in memory:

```js
tessera.check(program)            // diagnostics array; empty when valid
tessera.format(text)              // canonical text
tessera.ast(text)                 // JSON syntax tree (spec section 20.1)
tessera.render(tree)              // canonical text of a tree
tessera.grammar("gbnf")           // also "ast-schema", "edit-schema" (spec section 20.3)
tessera.exec(["edit", "main.tes", "--op", op], { files: { "main.tes": text } })
// any command: { exitCode, stdout, stderr, files } where `files` holds the files the command wrote
```

Scripts in a page:

```html
<script type="module" src="tessera.js?auto"></script>
<script type="text/tessera" data-output="#out">
fn main() -> Unit uses io {
print("from a script tag")
}
</script>
<pre id="out"></pre>
```

## The page

`run(program, { page: true })`, and every `<script type="text/tessera">`, gives the program the effect `page`: it reads and changes the document and reacts to events (spec section 21.5).

```tessera
fn toggle(event: PageEvent) -> Unit uses page {
if element_has_class(event.element, "open") { remove_element_class(event.element, "open") } else { add_element_class(event.element, "open") }
}

fn main() -> Unit uses page {
for item in page_select("li") {
on_event(item, "click", toggle)
}
}
```

Each event runs the program again, calling only its handler, which takes 1 to 3 ms. Keep the state in the page (classes, attributes, `store_text`). `on_event` writes an attribute naming the handler (`data-tessera-on-click="toggle"`), and the `Page` class in `tessera.js` turns that into a listener. A page that runs Tessera needs `'wasm-unsafe-eval'` in its Content-Security-Policy `script-src`. The Tessera website's scripts are an example: `wwwroot/assets/tes/site.tes` in the website repository.

## Limits

- A program runs on the calling thread until it ends. Run long programs in a Web Worker: the module works there unchanged.
- The browser's own stack allows about 940 nested calls in V8. Beyond the limit (700 by default) a program stops with R013.
- The interpreter is much slower than a compiled program. `set xs[i] = v` copies the list, so programs that update large buffers element by element are slow (spec section 19).
- `fs`, `net`, `db`, `process`, `ui` and `audio` are not available. `page` handlers work only in interpreted programs, not in `build --wasm` modules.

## Host interface

`tessera.wasm` imports six functions from the module `tessera`: `get`, `take`, `put`, `now`, `monotonic` and `random`. It exports `memory` and `tessera_main`. The keys and their meaning are in spec section 21.3 and at the top of `runtime/wasm.rs`. `tessera.js` implements them in `exec`. Another host (Node without the library, wasmtime, a server) needs only those six functions.
