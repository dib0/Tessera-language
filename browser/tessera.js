// Tessera in the browser (spec section 21). Loads tessera.wasm, the Tessera compiler built for
// WebAssembly (`tessera1 build stage1/main.tes --wasm -o browser/tessera.wasm`), and runs its
// command line against files kept in memory. `run` interprets a program, given as Tessera text or
// as its JSON syntax tree (spec section 20.1), in the effects io, env, time, random and log.
//
//   import { Tessera } from "./tessera.js";
//   const tessera = await Tessera.load();
//   const result = tessera.run('fn main() -> Unit uses io {\nprint("hello")\n}\n');
//   // result: { exitCode: 0, stdout: "hello\n", stderr: "", diagnostics: [] }
//
// No dependencies. Works in browsers, Web Workers, Deno and Node 18+.

const encoder = new TextEncoder();

// Nested calls the interpreter allows here. The JavaScript engine's own stack (about 1 MB in V8,
// roughly 1 KB per interpreted call) is far smaller than the 512 MB thread of a native
// `tessera1 interpret`; staying below it gives R013 with the program's location.
const DEFAULT_DEPTH = 700;

function bytesOf(value) {
  if (value instanceof Uint8Array) return value;
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  return encoder.encode(String(value));
}

function normalPath(path) {
  return path.startsWith("./") ? path.slice(2) : path;
}

export class Tessera {
  /** @param {WebAssembly.Module} module the compiled tessera.wasm */
  constructor(module) {
    this.module = module;
  }

  /**
   * Loads and compiles tessera.wasm (by default next to this file).
   * @param {string | URL | ArrayBuffer | Uint8Array | WebAssembly.Module} [source]
   */
  static async load(source = new URL("tessera.wasm", import.meta.url)) {
    if (source instanceof WebAssembly.Module) return new Tessera(source);
    if (source instanceof ArrayBuffer || source instanceof Uint8Array) {
      return new Tessera(await WebAssembly.compile(source));
    }
    const response = await fetch(source);
    if (!response.ok) throw new Error(`cannot load ${source}: ${response.status}`);
    if (WebAssembly.compileStreaming && (response.headers.get("content-type") || "").startsWith("application/wasm")) {
      return new Tessera(await WebAssembly.compileStreaming(response));
    }
    return new Tessera(await WebAssembly.compile(await response.arrayBuffer()));
  }

  /**
   * Runs the compiler's command line, as `tessera1 <args...>`, in a fresh instance.
   * @param {string[]} args e.g. ["check", "main.tes", "--json"]
   * @param {object} [options]
   * @param {Object<string, string | Uint8Array>} [options.files] the files the command can read
   * @param {string | Uint8Array} [options.stdin] standard input of the program
   * @param {Object<string, string>} [options.env] environment variables
   * @param {(text: string) => void} [options.onStdout] receives output as it is written
   * @param {(text: string) => void} [options.onStderr]
   * @param {Page} [options.page] the web page the `page` effect works on (spec section 21.5)
   * @returns {{exitCode: number, stdout: string, stderr: string, files: Object<string, string>}}
   */
  exec(args, options = {}) {
    const files = new Map();
    for (const [path, content] of Object.entries(options.files || {})) files.set(normalPath(path), bytesOf(content));
    const page = options.page || null;
    const env = { TESSERA_INTERPRET_DEPTH: String(DEFAULT_DEPTH), ...(page ? { TESSERA_PAGE: "1" } : {}), ...(options.env || {}) };
    const stdin = bytesOf(options.stdin || "");
    const written = new Set();
    const out = { stdout: "", stderr: "" };
    const decoders = { stdout: new TextDecoder(), stderr: new TextDecoder() };
    const listeners = { stdout: options.onStdout, stderr: options.onStderr };
    let exitCode = null;
    let staged = null;
    let memory = null;
    const view = () => new Uint8Array(memory.buffer);
    const text = (pointer, length) => new TextDecoder().decode(view().slice(pointer, pointer + length));

    const valueOf = (key) => {
      if (key === "args") return encoder.encode(args.join("\0"));
      if (key === "stdin") return stdin;
      if (key.startsWith("env/")) {
        const name = key.slice(4);
        return Object.prototype.hasOwnProperty.call(env, name) ? encoder.encode(env[name]) : null;
      }
      if (key.startsWith("file/")) return files.get(normalPath(key.slice(5))) || null;
      if (key.startsWith("page/") && page) {
        const [op, ...rest] = JSON.parse(key.slice(5));
        const answer = page.call(op, rest);
        return answer === null ? null : encoder.encode(answer);
      }
      return null;
    };

    const imports = {
      tessera: {
        get(keyPointer, keyLength) {
          staged = valueOf(text(keyPointer, keyLength));
          return staged === null ? -1 : staged.length;
        },
        take(destination) {
          view().set(staged, destination);
          staged = null;
        },
        put(keyPointer, keyLength, valuePointer, valueLength) {
          const key = text(keyPointer, keyLength);
          const value = view().slice(valuePointer, valuePointer + valueLength);
          if (key === "stdout" || key === "stderr") {
            const piece = decoders[key].decode(value, { stream: true });
            out[key] += piece;
            if (listeners[key] && piece) listeners[key](piece);
            return 0;
          }
          if (key === "exit") {
            exitCode = Number(new TextDecoder().decode(value));
            return 0;
          }
          if (key.startsWith("file/")) {
            const path = normalPath(key.slice(5));
            files.set(path, value);
            written.add(path);
            return 0;
          }
          return -1;
        },
        now: () => Date.now(),
        monotonic: () => performance.now(),
        random(destination, length) {
          for (let at = 0; at < length; at += 65536) {
            crypto.getRandomValues(view().subarray(destination + at, destination + Math.min(length, at + 65536)));
          }
        },
      },
    };

    const instance = new WebAssembly.Instance(this.module, imports);
    memory = instance.exports.memory;
    try {
      const code = instance.exports.tessera_main();
      if (exitCode === null) exitCode = code;
    } catch (error) {
      // `exit` and runtime errors end in a trap after the exit code was put. The engine's stack can
      // still run out before the interpreter's limit (deeply nested expressions): that is R013 too.
      if (exitCode === null) {
        const overflow = error instanceof RangeError || /call stack/i.test(String(error && error.message));
        const message = overflow
          ? "runtime error R013: recursion is too deep: the stack is full\n  at (the browser's stack ran out; lower TESSERA_INTERPRET_DEPTH)\n"
          : `internal error: ${error && error.message ? error.message : error}\n`;
        out.stderr += message;
        if (listeners.stderr) listeners.stderr(message);
        exitCode = 70;
      }
    }
    const changed = {};
    for (const path of written) changed[path] = new TextDecoder().decode(files.get(path));
    return { exitCode, stdout: out.stdout, stderr: out.stderr, files: changed };
  }

  /**
   * Interprets a program: Tessera text, or its syntax tree as an object or JSON text.
   * @param {string | object} program
   * @param {object} [options] as for `exec`, plus `args` (the program's arguments) and `modules`
   *   (further files by path, e.g. {"shapes.tes": "..."} for `import shapes`)
   * @returns {{exitCode: number, stdout: string, stderr: string, diagnostics: object[]}}
   *   `diagnostics` holds the compile errors (spec section 20.4) when the program did not run.
   */
  run(program, options = {}) {
    const entry = Tessera.entryFile(program);
    const files = { ...(options.modules || {}), [entry.path]: entry.content };
    const args = ["interpret", entry.path, "--", ...(options.args || [])];
    const page = options.page === true ? this.page() : options.page || null;
    if (page) page.remember({ path: entry.path, files, onStdout: options.onStdout, onStderr: options.onStderr });
    const result = this.exec(args, { ...options, files, page });
    let diagnostics = [];
    if (result.exitCode === 1 && /\d+ error\(s\)(; no code was generated|; the program was not run)\n$/.test(result.stderr)) {
      const report = this.exec(["interpret", entry.path, "--json"], { files });
      diagnostics = JSON.parse(report.stdout || "{}").diagnostics || [];
    }
    return { exitCode: result.exitCode, stdout: result.stdout, stderr: result.stderr, diagnostics };
  }

  /** The compile errors of a program (text or tree) as JSON objects; empty when it is valid. */
  check(program, modules = {}) {
    const entry = Tessera.entryFile(program);
    const result = this.exec(["check", entry.path, "--json"], { files: { ...modules, [entry.path]: entry.content } });
    return JSON.parse(result.stdout || "{}").diagnostics || [];
  }

  /** The canonical text of a program (text or tree); throws with the diagnostics when it cannot be read. */
  format(program) {
    const entry = Tessera.entryFile(program);
    if (entry.path.endsWith(".json")) return this.render(program);
    const result = this.exec(["format", entry.path], { files: { [entry.path]: entry.content } });
    if (result.exitCode !== 0) throw new Error(result.stderr);
    return result.files[entry.path] ?? entry.content;
  }

  /** The JSON syntax tree of Tessera text (spec section 20.1). */
  ast(source) {
    const result = this.exec(["ast", "main.tes"], { files: { "main.tes": source } });
    if (result.exitCode !== 0) throw new Error(result.stderr);
    return JSON.parse(result.stdout);
  }

  /** The canonical Tessera text of a JSON syntax tree. */
  render(tree) {
    const content = typeof tree === "string" ? tree : JSON.stringify(tree);
    const result = this.exec(["render", "main.json", "-o", "main.tes"], { files: { "main.json": content } });
    if (result.exitCode !== 0) throw new Error(result.stderr);
    return result.files["main.tes"];
  }

  /** The grammar for constrained decoding (spec section 20.3): "gbnf", "ast-schema" or "edit-schema". */
  grammar(format = "gbnf") {
    const result = this.exec(["grammar", "--format", format]);
    if (result.exitCode !== 0) throw new Error(result.stderr);
    return result.stdout;
  }

  /** Where a program goes in the in-memory files: a tree is `main.json`, text is `main.tes`. */
  static entryFile(program) {
    if (typeof program !== "string") return { path: "main.json", content: JSON.stringify(program) };
    return program.trimStart().startsWith("{") ? { path: "main.json", content: program } : { path: "main.tes", content: program };
  }

  /**
   * Runs every `<script type="text/tessera">` in the document. Output goes to the element named by
   * the script's `data-output` attribute (a CSS selector), or to the console.
   */
  async runScripts(root = document) {
    for (const script of root.querySelectorAll('script[type="text/tessera"]')) {
      const source = script.src ? await (await fetch(script.src)).text() : script.textContent.replace(/^\n/, "");
      const target = script.dataset.output ? root.querySelector(script.dataset.output) : null;
      const write = (piece, isError) => {
        if (target) target.append(document.createTextNode(piece));
        else (isError ? console.error : console.log)(piece.replace(/\n$/, ""));
      };
      this.run(source, { page: true, stdin: script.dataset.input || "", onStdout: (s) => write(s, false), onStderr: (s) => write(s, true) });
    }
  }

  /** The web page of this document, shared by all scripts run with `page: true`. */
  page() {
    if (!this.documentPage) this.documentPage = new Page(this, document);
    return this.documentPage;
  }
}

/**
 * The `page` effect (spec section 21.5): elements by id, the operations the runtime asks for
 * (runtime/page.rs), and the handlers. `on_event` and `on_timer` set an attribute naming the handler
 * function (`data-tessera-on-<kind>`, `data-tessera-timer`); the page turns that into a listener that
 * runs the program again with `interpret --handler <function>`, the event described in environment
 * variables. A run takes a few milliseconds; the state of a page lives in the page itself.
 */
export class Page {
  constructor(tessera, doc) {
    this.tessera = tessera;
    this.document = doc;
    this.elements = [null];
    this.ids = new WeakMap();
    this.handlers = new WeakMap();
    this.program = null;
    this.event = null;
    this.visible = null;
  }

  /** The program whose handlers are being registered (the one running now). */
  remember(program) {
    this.program = program;
  }

  id(node) {
    if (!node) return 0;
    let id = this.ids.get(node);
    if (id === undefined) {
      id = this.elements.length;
      this.elements.push(node);
      this.ids.set(node, id);
    }
    return id;
  }

  node(id) {
    return this.elements[Number(id)] || null;
  }

  list(nodes) {
    return Array.from(nodes, (n) => this.id(n)).join("\0");
  }

  /** One operation of runtime/page.rs: the answer as text, or null for "none". */
  call(op, args) {
    const node = this.node(args[0]);
    switch (op) {
      case "page_select":
        try { return this.list(this.document.querySelectorAll(args[0])); } catch { return ""; }
      case "element_select":
        try { return node ? this.list(node.querySelectorAll(args[1])) : ""; } catch { return ""; }
      case "element_text":
        return node ? node.textContent : "";
      case "set_element_text":
        if (node) node.textContent = args[1];
        return "";
      case "element_attribute":
        return node && node.hasAttribute(args[1]) ? node.getAttribute(args[1]) : null;
      case "set_element_attribute":
        if (node) {
          node.setAttribute(args[1], args[2]);
          this.watch(node, args[1], args[2]);
        }
        return "";
      case "remove_element_attribute":
        if (node) node.removeAttribute(args[1]);
        return "";
      case "element_has_class":
        return node && node.classList.contains(args[1]) ? "true" : "false";
      case "add_element_class":
        if (node) node.classList.add(args[1]);
        return "";
      case "remove_element_class":
        if (node) node.classList.remove(args[1]);
        return "";
      case "set_element_style":
        if (node) node.style.setProperty(args[1], args[2]);
        return "";
      case "create_element":
        return String(this.id(this.document.createElement(args[0])));
      case "append_element": {
        const child = this.node(args[1]);
        if (node && child) node.appendChild(child);
        return "";
      }
      case "insert_element_before": {
        const child = this.node(args[1]);
        const before = this.node(args[2]);
        if (node && child) node.insertBefore(child, before && before.parentNode === node ? before : null);
        return "";
      }
      case "prevent_default":
        if (this.event && this.event.preventDefault) this.event.preventDefault();
        return "";
      case "page_confirm":
        return typeof confirm === "function" && confirm(args[0]) ? "true" : "false";
      case "copy_to_clipboard":
        if (typeof navigator !== "undefined" && navigator.clipboard) navigator.clipboard.writeText(args[0]).catch(() => {});
        return "";
      case "media_matches":
        return typeof matchMedia === "function" && matchMedia(args[0]).matches ? "true" : "false";
      case "stored_text":
        try { return localStorage.getItem(args[0]); } catch { return null; }
      case "store_text":
        try { localStorage.setItem(args[0], args[1]); } catch { /* storage can be blocked */ }
        return "";
      default:
        return null;
    }
  }

  /** An attribute naming a handler becomes a listener (or a timer) for the running program. */
  watch(node, name, value) {
    const program = this.program;
    if (!program) return;
    if (name === "data-tessera-timer") {
      const space = value.indexOf(" ");
      const ms = Number(value.slice(0, space));
      const handler = value.slice(space + 1);
      setTimeout(() => {
        if (node.getAttribute("data-tessera-timer") === value) node.removeAttribute("data-tessera-timer");
        this.dispatch(program, handler, "timer", node, null);
      }, ms);
      return;
    }
    if (!name.startsWith("data-tessera-on-")) return;
    const kind = name.slice("data-tessera-on-".length);
    let kinds = this.handlers.get(node);
    if (!kinds) this.handlers.set(node, (kinds = new Map()));
    const known = kinds.has(kind);
    kinds.set(kind, { program, handler: value });
    if (known) return;
    const current = () => kinds.get(kind);
    if (kind === "visible") {
      if (typeof IntersectionObserver === "undefined") {
        this.dispatch(program, value, "visible", node, null);
        return;
      }
      if (!this.visible) {
        this.visible = new IntersectionObserver((entries) => {
          for (const entry of entries) {
            const registered = this.handlers.get(entry.target)?.get("visible");
            // Once per element: the first time it scrolls into view.
            if (entry.isIntersecting && registered) {
              this.visible.unobserve(entry.target);
              this.dispatch(registered.program, registered.handler, "visible", entry.target, null);
            }
          }
        }, { rootMargin: "0px 0px -8% 0px", threshold: 0.08 });
      }
      this.visible.observe(node);
      return;
    }
    if (kind === "pointermove") {
      // At most one run per 16 ms, and always one for the last move: pointer events can come faster
      // than the screen is drawn.
      let last = -Infinity;
      let pending = null;
      const run = (event) => {
        last = performance.now();
        this.dispatch(current().program, current().handler, kind, node, event);
      };
      node.addEventListener(kind, (event) => {
        const wait = last + 16 - performance.now();
        if (wait <= 0 && pending === null) return run(event);
        if (pending !== null) clearTimeout(pending.timer);
        pending = { timer: setTimeout(() => { pending = null; run(event); }, Math.max(wait, 0)) };
      });
      return;
    }
    node.addEventListener(kind, (event) => this.dispatch(current().program, current().handler, kind, node, event));
  }

  /** Runs `handler` of `program` for an event on `node`. */
  dispatch(program, handler, kind, node, event) {
    const box = node.getBoundingClientRect ? node.getBoundingClientRect() : { left: 0, top: 0, width: 0, height: 0 };
    const env = {
      TESSERA_EVENT_KIND: kind,
      TESSERA_EVENT_ELEMENT: String(this.id(node)),
      TESSERA_EVENT_TARGET: String(this.id(event && event.target && event.target.nodeType === 1 ? event.target : node)),
      TESSERA_EVENT_X: String(event && "clientX" in event ? event.clientX - box.left : 0),
      TESSERA_EVENT_Y: String(event && "clientY" in event ? event.clientY - box.top : 0),
      TESSERA_EVENT_WIDTH: String(box.width),
      TESSERA_EVENT_HEIGHT: String(box.height),
      TESSERA_EVENT_KEY: (event && event.key) || "",
      TESSERA_EVENT_POINTER: (event && event.pointerType) || "",
    };
    const previous = [this.program, this.event];
    this.program = program;
    this.event = event;
    try {
      const result = this.tessera.exec(["interpret", program.path, "--handler", handler], { files: program.files, page: this, env, onStdout: program.onStdout, onStderr: program.onStderr || ((s) => console.error(s.replace(/\n$/, ""))) });
      if (result.exitCode !== 0 && !program.onStderr && !result.stderr) console.error(`tessera: handler ${handler} ended with exit code ${result.exitCode}`);
    } finally {
      [this.program, this.event] = previous;
    }
  }
}

// `<script type="module" src="tessera.js?auto">` runs the page's Tessera scripts once it has loaded.
if (typeof document !== "undefined" && new URL(import.meta.url).searchParams.has("auto")) {
  const start = async () => (await Tessera.load()).runScripts();
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
}
