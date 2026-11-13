"""Mutation testing of the harness tasks: how strong are the test sets?

Each mutant is the reference solution with one small change (a comparison flipped, `+` and `-`
swapped, an integer literal moved by one, `and`/`or` or `true`/`false` swapped, a `not` removed).
A mutant that does not compile is stillborn and not counted. A compiled mutant is *killed* by a
test set when at least one test of the set fails on it. The mutation score of a set is
killed / compiled mutants. Surviving mutants are either equivalent to the solution (no test can
kill them) or show a gap in the tests; `--limit` samples mutants with a fixed seed.

Used through `harness.py mutate`; the report goes to standard output and to
harness/results/mutation-<stamp>/.
"""
from __future__ import annotations

import concurrent.futures
import datetime
import json
import os
import random
import re
import shutil
import tempfile

SWAPS = {"<": "<=", "<=": "<", ">": ">=", ">=": ">", "==": "!=", "!=": "==",
         "+": "-", "-": "+", "+=": "-=", "-=": "+=", "*": "/", "and": "or", "or": "and",
         "true": "false", "false": "true"}
TOKEN = re.compile(r"->|=>|<=|>=|==|!=|\+=|-=|[<>+\-*]|[A-Za-z_][A-Za-z0-9_]*|\d[\d_]*(?:\.\d+)?|\S")


def code_spans(source: str):
    """(start, end) of the parts of each line that are code: outside text literals and comments.
    Interpolations inside text are left alone too (mutating them mostly changes output text)."""
    for match in re.finditer(r"[^\n]*", source):
        line, base = match.group(0), match.start()
        i, start, n = 0, 0, len(line)
        while i < n:
            ch = line[i]
            if ch == '"':
                if start < i:
                    yield base + start, base + i
                i += 1
                while i < n and line[i] != '"':
                    i += 2 if line[i] == "\\" else 1
                i += 1
                start = i
                continue
            if line.startswith("//", i):
                break
            i += 1
        if start < min(i, n):
            yield base + start, base + min(i, n)


def mutants(source: str) -> list[tuple[int, int, str, str]]:
    """(start, end, original, replacement) for every mutation point in one file."""
    found = []
    for lo, hi in code_spans(source):
        tokens = list(TOKEN.finditer(source, lo, hi))
        for k, tok in enumerate(tokens):
            text = tok.group(0)
            prev = tokens[k - 1].group(0) if k else ""
            if text in ("+", "-"):
                # Only binary + and -: the previous token ends a value.
                if not (re.fullmatch(r"[A-Za-z0-9_]+", prev) or prev in (")", "]")) or prev in SWAPS or prev in ("return", "else", "in", "not", "try", "if", "while", "set", "let"):
                    continue
            if text in SWAPS:
                found.append((tok.start(), tok.end(), text, SWAPS[text]))
            elif text == "not":
                found.append((tok.start(), tok.end() + 1 if source[tok.end():tok.end() + 1] == " " else tok.end(), text, ""))
            elif re.fullmatch(r"\d[\d_]*", text):
                value = int(text.replace("_", ""))
                found.append((tok.start(), tok.end(), text, str(value + 1)))
                if value > 0:
                    found.append((tok.start(), tok.end(), text, str(value - 1)))
    return found


def evaluate(h, task, files: dict[str, str]) -> dict:
    workdir = tempfile.mkdtemp(prefix=f"tessera-mut-{task.id}-")
    try:
        h.write_program(files, workdir)
        if h.build_program(workdir).returncode != 0:
            return {"stillborn": True}
        binary = h.binary_path(workdir)
        feedback = any(not h.run_test(binary, t, task, workdir)[0] for t in task.tests)
        heldout = any(not h.run_test(binary, t, task, workdir)[0] for t in (task.heldout or []))
        return {"stillborn": False, "feedback": feedback, "heldout": heldout}
    finally:
        shutil.rmtree(workdir, ignore_errors=True)


def main(args, h) -> None:
    tasks = h.load_tasks(args.tasks)
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    out_dir = os.path.join(h.HARNESS, "results", f"mutation-{stamp}")
    os.makedirs(out_dir)
    jobs = []
    for task in tasks:
        solution = task.solution_files if task.multi_file else {"main.tes": task.solution}
        points = [(path, m) for path, source in solution.items() for m in mutants(source)]
        if args.limit and len(points) > args.limit:
            points = random.Random(f"{task.id}-mutants").sample(points, args.limit)
        for path, (start, end, old, new) in points:
            files = dict(solution)
            files[path] = solution[path][:start] + new + solution[path][end:]
            line = solution[path].count("\n", 0, start) + 1
            jobs.append((task, files, {"file": path, "line": line, "from": old, "to": new}))
    print(f"{len(jobs)} mutants over {len(tasks)} tasks", flush=True)

    def work(job):
        task, files, where = job
        return task.id, where, evaluate(h, task, files)

    results: dict[str, list] = {t.id: [] for t in tasks}
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for done, (task_id, where, outcome) in enumerate(pool.map(work, jobs), 1):
            results[task_id].append({**where, **outcome})
            if done % 100 == 0:
                print(f"  {done}/{len(jobs)}", flush=True)
    with open(os.path.join(out_dir, "mutants.json"), "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2)

    lines = ["| Task | Suite | Feedback tests | Held-out tests | Mutants compiled | Killed by feedback tests | Killed by held-out tests | Killed by either | Survived |",
             "|---|---|---|---|---|---|---|---|---|"]
    totals: dict[str, list[int]] = {}
    for task in tasks:
        live = [r for r in results[task.id] if not r["stillborn"]]
        n = len(live)
        kf = sum(r["feedback"] for r in live)
        kh = sum(r["heldout"] for r in live)
        ke = sum(r["feedback"] or r["heldout"] for r in live)
        acc = totals.setdefault(task.suite, [0, 0, 0, 0])
        for i, v in enumerate((n, kf, kh, ke)):
            acc[i] += v
        pct = lambda k: f"{k / n * 100:.0f}%" if n else "-"
        lines.append(f"| {task.id} | {task.suite} | {len(task.tests)} | {len(task.heldout or [])} | {n} | {pct(kf)} | {pct(kh)} | {pct(ke)} | {n - ke} |")
    for suite, (n, kf, kh, ke) in sorted(totals.items()):
        pct = lambda k: f"{k / n * 100:.1f}%" if n else "-"
        lines.append(f"| **all {suite}** | | | | {n} | {pct(kf)} | {pct(kh)} | {pct(ke)} | {n - ke} |")
    lines.append("\nMutation score = killed / compiled mutants. Survivors include equivalent mutants, which no test can kill.")
    text = "\n".join(lines) + "\n"
    with open(os.path.join(out_dir, "summary.md"), "w", encoding="utf-8") as f:
        f.write(text)
    print("\n" + text)
    print(f"results: {os.path.relpath(out_dir, h.REPO)}")
