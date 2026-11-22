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

// Nested calls the interpreter allows here; the browser's WebAssembly stack is smaller than the
// 512 MB thread a native `tessera1 interpret` uses.
const DEFAULT_DEPTH = 2000;

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
   * @returns {{exitCode: number, stdout: string, stderr: string, files: Object<string, string>}}
   */
  exec(args, options = {}) {
    const files = new Map();
    for (const [path, content] of Object.entries(options.files || {})) files.set(normalPath(path), bytesOf(content));
    const env = { TESSERA_INTERPRET_DEPTH: String(DEFAULT_DEPTH), ...(options.env || {}) };
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
      // `exit` and runtime errors end in a trap after the exit code was put; anything else is a bug.
      if (exitCode === null) {
        const message = `internal error: ${error && error.message ? error.message : error}\n`;
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
    const result = this.exec(args, { ...options, files });
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
      this.run(source, { stdin: script.dataset.input || "", onStdout: (s) => write(s, false), onStderr: (s) => write(s, true) });
    }
  }
}

// `<script type="module" src="tessera.js?auto">` runs the page's Tessera scripts once it has loaded.
if (typeof document !== "undefined" && new URL(import.meta.url).searchParams.has("auto")) {
  const start = async () => (await Tessera.load()).runScripts();
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
}
