#!/usr/bin/env python3
"""Independent checks of the exported grammars (spec section 20.3), without dependencies.

1. Every canonical Tessera file in the repository is accepted by the GBNF grammar
   (`tessera1 grammar --format gbnf`), checked with a small Earley recognizer.
2. The JSON syntax tree of every file (`tessera1 ast`) validates against the recursive
   AST schema, and against the unrolled schema (`--depth N`) when it nests less than N.
3. The unrolled schema contains no `$ref` cycle (Claude's structured outputs reject recursion)
   and every object in all schemas has `additionalProperties: false`.
4. Sample edit operations validate against the edit schema, and malformed ones do not.

usage: check_grammar.py [--compiler bin/tessera1] [--max-bytes N] [files...]
"""
import json
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


# ---------- GBNF ----------

def parse_gbnf(text):
    """Returns {rule: [alternative, ...]}, an alternative being a list of symbols.
    A symbol is ('n', name) or ('c', predicate)."""
    rules = {}
    body = re.sub(r"#[^\n]*", "", text)
    defs = re.split(r"\n(?=[a-z][a-z0-9-]* ::=)", body.strip())
    counter = [0]

    def fresh():
        counter[0] += 1
        return f"_g{counter[0]}"

    for d in defs:
        name, rhs = d.split("::=", 1)
        tokens = tokenize(rhs)
        pos, alts = parse_alts(tokens, 0, rules, fresh)
        assert pos == len(tokens), (name, tokens[pos:])
        rules[name.strip()] = alts
    return rules


def tokenize(rhs):
    tokens, i = [], 0
    while i < len(rhs):
        c = rhs[i]
        if c.isspace():
            i += 1
        elif c == '"':
            j, s = i + 1, ""
            while rhs[j] != '"':
                if rhs[j] == "\\":
                    s += {"n": "\n", "t": "\t", '"': '"', "\\": "\\"}[rhs[j + 1]]
                    j += 2
                else:
                    s += rhs[j]
                    j += 1
            tokens.append(("lit", s))
            i = j + 1
        elif c == "[":
            j = i + 1
            while rhs[j] != "]" or rhs[j - 1] == "\\" and rhs[j - 2] != "\\":
                j += 1
            tokens.append(("class", rhs[i + 1:j]))
            i = j + 1
        elif c in "()|*+?":
            tokens.append((c, c))
            i += 1
        else:
            m = re.match(r"[a-z][a-z0-9-]*", rhs[i:])
            tokens.append(("name", m.group(0)))
            i += len(m.group(0))
    return tokens


def class_predicate(spec):
    negate = spec.startswith("^")
    if negate:
        spec = spec[1:]
    chars, ranges, i = set(), [], 0
    while i < len(spec):
        c = spec[i]
        if c == "\\":
            c = {"n": "\n", "t": "\t"}.get(spec[i + 1], spec[i + 1])
            i += 2
        else:
            i += 1
        if i + 1 < len(spec) and spec[i] == "-":
            ranges.append((c, spec[i + 1]))
            i += 2
        else:
            chars.add(c)
    return lambda ch: (ch in chars or any(a <= ch <= b for a, b in ranges)) != negate


def parse_alts(tokens, pos, rules, fresh):
    alts = []
    pos, seq = parse_seq(tokens, pos, rules, fresh)
    alts.append(seq)
    while pos < len(tokens) and tokens[pos][0] == "|":
        pos, seq = parse_seq(tokens, pos + 1, rules, fresh)
        alts.append(seq)
    return pos, alts


def parse_seq(tokens, pos, rules, fresh):
    seq = []
    while pos < len(tokens) and tokens[pos][0] not in ("|", ")"):
        kind, value = tokens[pos]
        if kind == "lit":
            atom = [("c", (lambda x: lambda ch: ch == x)(ch)) for ch in value]
            pos += 1
        elif kind == "class":
            atom = [("c", class_predicate(value))]
            pos += 1
        elif kind == "name":
            atom = [("n", value)]
            pos += 1
        elif kind == "(":
            pos, alts = parse_alts(tokens, pos + 1, rules, fresh)
            assert tokens[pos][0] == ")"
            pos += 1
            g = fresh()
            rules[g] = alts
            atom = [("n", g)]
        else:
            raise ValueError(tokens[pos])
        if pos < len(tokens) and tokens[pos][0] in "*+?":
            op = tokens[pos][0]
            pos += 1
            inner = fresh()
            rules[inner] = [atom]
            r = fresh()
            if op == "?":
                rules[r] = [[], [("n", inner)]]
            elif op == "*":
                rules[r] = [[], [("n", inner), ("n", r)]]
            else:
                rules[r] = [[("n", inner)], [("n", inner), ("n", r)]]
            atom = [("n", r)]
        seq.extend(atom)
    return pos, seq


def nullable_rules(rules):
    nullable, changed = set(), True
    while changed:
        changed = False
        for name, alts in rules.items():
            if name not in nullable and any(all(s[0] == "n" and s[1] in nullable for s in alt) for alt in alts):
                nullable.add(name)
                changed = True
    return nullable


def recognize(rules, nullable, text, start="root"):
    """Earley recognizer over characters. Items are (rule, alt, dot, origin)."""
    n = len(text)
    sets = [dict() for _ in range(n + 1)]

    def add(i, item):
        if item not in sets[i]:
            sets[i][item] = True
            return True
        return False

    for a in range(len(rules[start])):
        add(0, (start, a, 0, 0))
    for i in range(n + 1):
        agenda = list(sets[i].keys())
        waiting = {}
        completed = set()
        while agenda:
            item = agenda.pop()
            name, a, dot, origin = item
            alt = rules[name][a]
            if dot == len(alt):
                if origin == i:
                    completed.add(name)
                for parent in (list(sets[origin].keys()) if origin != i else list(waiting.get(name, []))):
                    pn, pa, pd, po = parent
                    palt = rules[pn][pa]
                    if pd < len(palt) and palt[pd] == ("n", name):
                        new = (pn, pa, pd + 1, po)
                        if add(i, new):
                            agenda.append(new)
                continue
            kind, value = alt[dot]
            if kind == "n":
                waiting.setdefault(value, set()).add(item)
                for b in range(len(rules[value])):
                    new = (value, b, 0, i)
                    if add(i, new):
                        agenda.append(new)
                if value in nullable or value in completed:
                    new = (name, a, dot + 1, origin)
                    if add(i, new):
                        agenda.append(new)
            elif i < n and value(text[i]):
                add(i + 1, (name, a, dot + 1, origin))
        if i < n and not sets[i + 1]:
            return i
    if any(nm == start and d == len(rules[nm][a]) and o == 0 for (nm, a, d, o) in sets[n]):
        return -1
    return n


# ---------- JSON Schema (the subset the exported schemas use) ----------

def validate(schema, value, root, path="$"):
    if "$ref" in schema:
        return validate(root["$defs"][schema["$ref"].split("/")[-1]], value, root, path)
    if "anyOf" in schema:
        errors = []
        for option in schema["anyOf"]:
            e = validate(option, value, root, path)
            if not e:
                return []
            errors.append(e)
        return [f"{path}: matches no alternative ({min(errors, key=len)[0]})"]
    t = schema.get("type")
    if t == "object":
        if not isinstance(value, dict):
            return [f"{path}: not an object"]
        props = schema.get("properties", {})
        for key in schema.get("required", []):
            if key not in value:
                return [f"{path}: missing {key}"]
        for key, v in value.items():
            if key not in props:
                return [f"{path}: unexpected {key}"]
            e = validate(props[key], v, root, f"{path}.{key}")
            if e:
                return e
        return []
    if t == "array":
        if not isinstance(value, list):
            return [f"{path}: not an array"]
        for k, v in enumerate(value):
            e = validate(schema["items"], v, root, f"{path}[{k}]")
            if e:
                return e
        return []
    checks = {"string": lambda v: isinstance(v, str), "integer": lambda v: isinstance(v, int) and not isinstance(v, bool),
              "boolean": lambda v: isinstance(v, bool)}
    if t in checks and not checks[t](value):
        return [f"{path}: not a {t}"]
    if "const" in schema and value != schema["const"]:
        return [f"{path}: not {schema['const']!r}"]
    if "enum" in schema and value not in schema["enum"]:
        return [f"{path}: {value!r} not in enum"]
    return []


def refs_of(schema):
    if isinstance(schema, dict):
        found = [schema["$ref"].split("/")[-1]] if "$ref" in schema else []
        for v in schema.values():
            found += refs_of(v)
        return found
    if isinstance(schema, list):
        return [r for v in schema for r in refs_of(v)]
    return []


def has_cycle(schema):
    graph = {name: set(refs_of(d)) for name, d in schema["$defs"].items()}
    state = {}

    def visit(node):
        state[node] = 1
        for nxt in graph[node]:
            if state.get(nxt) == 1 or (nxt not in state and visit(nxt)):
                return True
        state[node] = 2
        return False

    return any(name not in state and visit(name) for name in graph)


def objects_closed(schema):
    if isinstance(schema, dict):
        if schema.get("type") == "object" and schema.get("additionalProperties") is not False:
            return False
        return all(objects_closed(v) for v in schema.values())
    if isinstance(schema, list):
        return all(objects_closed(v) for v in schema)
    return True


def nesting(value):
    if isinstance(value, dict):
        inner = max([nesting(v) for v in value.values()] or [0])
        return inner + (1 if "kind" in value else 0)
    if isinstance(value, list):
        return max([nesting(v) for v in value] or [0])
    return 0


# ---------- driver ----------

def run(compiler, *args):
    return subprocess.run([compiler, *args], capture_output=True, text=True)


def default_files():
    listed = subprocess.run(["git", "ls-files", "*.tes"], cwd=ROOT, capture_output=True, text=True).stdout.split()
    return [os.path.join(ROOT, f) for f in listed if not f.endswith("runtime_text.tes") and not f.startswith("harness/")]


def main():
    args = sys.argv[1:]
    compiler = os.path.join(ROOT, "bin", "tessera1")
    max_bytes = 12000
    depth = 24
    if "--compiler" in args:
        i = args.index("--compiler")
        compiler = args[i + 1]
        del args[i:i + 2]
    if "--max-bytes" in args:
        i = args.index("--max-bytes")
        max_bytes = int(args[i + 1])
        del args[i:i + 2]
    files = args or default_files()

    rules = parse_gbnf(run(compiler, "grammar", "--format", "gbnf").stdout)
    nullable = nullable_rules(rules)
    ast_schema = json.loads(run(compiler, "grammar", "--format", "ast-schema").stdout)
    unrolled = json.loads(run(compiler, "grammar", "--format", "ast-schema", "--depth", str(depth)).stdout)
    edit_schema = json.loads(run(compiler, "grammar", "--format", "edit-schema").stdout)
    failures = 0

    if has_cycle(unrolled):
        print("FAIL: the unrolled AST schema has a $ref cycle")
        failures += 1
    for name, schema in (("ast", ast_schema), ("unrolled", unrolled), ("edit", edit_schema)):
        if not objects_closed(schema):
            print(f"FAIL: an object in the {name} schema allows additional properties")
            failures += 1

    texts = grammar_checked = schema_checked = unrolled_checked = 0
    with tempfile.TemporaryDirectory() as tmp:
        for path in files:
            copy = os.path.join(tmp, "f.tes")
            with open(path, encoding="utf-8") as f:
                original = f.read()
            with open(copy, "w", encoding="utf-8") as f:
                f.write(original)
            if run(compiler, "format", copy).returncode != 0:
                continue  # files with syntax errors on purpose
            with open(copy, encoding="utf-8") as f:
                canonical = f.read()
            texts += 1
            tree = json.loads(run(compiler, "ast", copy).stdout)
            errors = validate(ast_schema, tree, ast_schema)
            if errors:
                print(f"FAIL schema {os.path.relpath(path, ROOT)}: {errors[0]}")
                failures += 1
            schema_checked += 1
            if nesting(tree) < depth:
                errors = validate(unrolled, tree, unrolled)
                if errors:
                    print(f"FAIL unrolled schema {os.path.relpath(path, ROOT)}: {errors[0]}")
                    failures += 1
                unrolled_checked += 1
            # Tests that must fail on purpose break naming rules the grammar also encodes.
            expected_failure = re.search(r"(compile_fail|stage1_fail|/fail/)", path) is not None
            if len(canonical.encode()) <= max_bytes and not expected_failure:
                stop = recognize(rules, nullable, canonical)
                if stop != -1:
                    line = canonical.count("\n", 0, stop) + 1
                    shown = canonical.splitlines()[line - 1] if line <= len(canonical.splitlines()) else ""
                    print(f"FAIL grammar {os.path.relpath(path, ROOT)}:{line}: {shown!r}")
                    failures += 1
                grammar_checked += 1

    good = [{"op": "add_fn", "source": "fn f() -> Int = 1"}, [{"op": "rename_symbol", "from": "a", "to": "b"}, {"op": "set_effect", "fn": "main", "uses": ["io"]}]]
    bad = [{"op": "add_fn"}, {"op": "set_effect", "fn": "main", "uses": ["disk"]}, {"op": "rename_symbol", "from": "a", "to": "b", "extra": 1}]
    for sample in good:
        if validate(edit_schema, sample, edit_schema):
            print(f"FAIL edit schema rejects {sample}")
            failures += 1
    for sample in bad:
        if not validate(edit_schema, sample, edit_schema):
            print(f"FAIL edit schema accepts {sample}")
            failures += 1

    print(f"{texts} canonical files: {grammar_checked} checked against the GBNF grammar, "
          f"{schema_checked} against the AST schema, {unrolled_checked} against the schema unrolled to depth {depth}; "
          f"{failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
