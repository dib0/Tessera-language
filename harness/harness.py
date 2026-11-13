#!/usr/bin/env python3
"""Tessera language measurement harness.

Measures how language variants affect (1) the token cost of Tessera code and
(2) how well a model writes Tessera: compile success, test success, fix rounds
and tokens spent. See harness/README.md.

Commands:
  tokens    Count real Claude tokens of existing Tessera code per variant.
  run       Let the model solve the tasks per variant; compile, test, feed back.
  dry-run   Same pipeline with the reference solutions instead of the model (no API).
  report    Summarise one or more result directories.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import datetime
import glob
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading
from dataclasses import dataclass, field

HARNESS = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HARNESS)
# The compiler carries the system's own extension: tessera1.exe on Windows.
TESSERA = os.path.join(REPO, "bin", "tessera1.exe" if os.name == "nt" else "tessera1")
DEFAULT_MODEL = "claude-opus-5-5"
MAX_ATTEMPTS = 4  # first answer + 3 fix rounds
COMPILE_TIMEOUT = 180
RUN_TIMEOUT = 10

# ---------------------------------------------------------------------------
# Variants
# ---------------------------------------------------------------------------


@dataclass
class Variant:
    name: str
    indent: int = 4
    features: list[str] = field(default_factory=list)
    # With `reference: false` the model gets no language reference at all, only
    # the harness rules its answer must follow to be gradeable. That measures
    # what a model already knows about Tessera, which is the one number that
    # says whether Tessera has reached anyone's training data; expect 0% until
    # it has. The two `extend-` tasks still show Tessera in their starter code,
    # so they are the easiest and will move first.
    reference: bool = True


def load_variants() -> dict[str, Variant]:
    with open(os.path.join(HARNESS, "variants.json"), encoding="utf-8") as f:
        raw = json.load(f)
    return {name: Variant(name=name, **spec) for name, spec in raw.items()}


OPEN, CLOSE = "{([", "})]"

def brackets(line: str) -> list[bool]:
    """The brackets of a line in order (True = opening), ignoring text literals, interpolation and comments."""
    found, i, n = [], 0, len(line)
    modes = []  # stack: "text", or an int = paren depth inside an interpolation
    while i < n:
        ch = line[i]
        if modes and modes[-1] == "text":
            if ch == "\\" and i + 1 < n and line[i + 1] == "(":
                modes.append(0); i += 2; continue
            if ch == "\\":
                i += 2; continue
            if ch == '"':
                modes.pop()
            i += 1; continue
        if ch == '"':
            modes.append("text")
        elif ch == "/" and line[i:i + 2] == "//":
            break
        elif modes and ch == "(":
            modes[-1] += 1
        elif modes and ch == ")":
            if modes[-1] == 0:
                modes.pop()
            else:
                modes[-1] -= 1
        elif ch in OPEN:
            found.append(True)
        elif ch in CLOSE:
            found.append(False)
        i += 1
    return found

def reindent(code: str, width: int) -> str:
    """Indent Tessera code by nesting: one level per line that has a bracket still open, and a
    line that starts with a closing bracket at the level of the line that opened it.
    Existing indentation is ignored, so any input width works."""
    open_lines: list[int] = []  # line number of each unclosed bracket
    levels: list[int] = []      # indentation level of each line
    out = []
    for number, line in enumerate(code.split("\n")):
        text = line.lstrip(" ")
        marks = brackets(text)
        leading = len(text) - len(text.lstrip("})]"))
        opener = None
        for _ in range(min(leading, len(open_lines))):
            opener = open_lines.pop()
        level = levels[opener] if opener is not None else len(set(open_lines))
        levels.append(level)
        out.append(" " * (level * width) + text if text else "")
        for opening in marks[leading:]:
            if opening:
                open_lines.append(number)
            elif open_lines:
                open_lines.pop()
    return "\n".join(out)


def reindent_markdown(text: str, width: int) -> str:
    """Re-indent only the ```tessera code blocks of a markdown document."""
    def fix(match: re.Match) -> str:
        return match.group(1) + reindent(match.group(2), width) + match.group(3)
    return re.sub(r"(```tessera\n)(.*?)(```)", fix, text, flags=re.S)


def indent_rule(width: int) -> str:
    if width == 0:
        return "Do not indent code: every line starts in the first column. Indentation has no meaning in Tessera."
    return f"Indent nested blocks with {width} spaces per level."


def render_reference(variant: Variant) -> str:
    with open(os.path.join(HARNESS, "reference", "tessera.md"), encoding="utf-8") as f:
        text = f.read()
    for feature in variant.features:
        with open(os.path.join(HARNESS, "reference", "features", feature + ".md"), encoding="utf-8") as f:
            text += "\n" + f.read()
    return reindent_markdown(text, variant.indent)


def system_prompt(variant: Variant) -> str:
    rules = (
        "Rules for every answer:\n"
        "- Reply with the complete program in a single ```tessera code block. Text outside the block is ignored.\n"
        "- The program is one file with a `main` function, or `handle` when the task asks for a web application. Its entry function may only use the effects `io` and `env` (a web application: none).\n"
    )
    last = "- Output must match the task exactly, including spacing and line breaks.\n"
    if not variant.reference:
        # Nothing that teaches the language, not even how it is laid out.
        return (
            "You write programs in Tessera, a programming language. You are given no reference for it: "
            "write it from what you already know. You have no way to run code yourself; a test harness "
            "compiles and runs your program and reports compiler errors or failing tests back to you.\n\n"
            + rules + last
        )
    return (
        "You write programs in Tessera, a programming language described in the reference below. "
        "You have no other documentation and no way to run code yourself; a test harness compiles "
        "and runs your program and reports compiler errors or failing tests back to you.\n\n"
        + rules
        + f"- {indent_rule(variant.indent)}\n"
        + last + "\n"
        + "<tessera_reference>\n" + render_reference(variant) + "\n</tessera_reference>"
    )


# ---------------------------------------------------------------------------
# Tasks
# ---------------------------------------------------------------------------


@dataclass
class Task:
    """A task has three test sets, kept apart (see README, "Test sets"):
    `tests[:examples]` are shown in the prompt; all of `tests` (examples and feedback tests) drive the
    repair loop; `heldout` (heldout.json, frozen by heldout.lock) is never shown and never fed back,
    and is scored once on the final program."""
    id: str
    kind: str
    prompt: str
    tests: list[dict]
    effects: list[str]
    solution: str            # single-file tasks; for multi-file tasks the entry file
    starter: str | None
    suite: str = "core"
    examples: int = 2
    heldout: list[dict] | None = None
    policy: list[dict] = field(default_factory=list)       # frozen source checks, scored with the held-out set
    starter_files: dict[str, str] | None = None             # multi-file tasks: path -> source
    solution_files: dict[str, str] | None = None

    @property
    def multi_file(self) -> bool:
        return self.starter_files is not None


def read_tree(directory: str) -> dict[str, str]:
    files = {}
    for path in sorted(glob.glob(os.path.join(directory, "**", "*.tes"), recursive=True)):
        with open(path, encoding="utf-8") as f:
            files[os.path.relpath(path, directory).replace(os.sep, "/")] = f.read()
    return files


HELDOUT_LOCK = os.path.join(HARNESS, "heldout.lock")


def file_hash(path: str) -> str:
    import hashlib
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def heldout_hashes() -> dict[str, str]:
    return {os.path.basename(os.path.dirname(p)): file_hash(p)
            for p in sorted(glob.glob(os.path.join(HARNESS, "tasks", "*", "heldout.json")))}


def check_frozen(tasks: list["Task"]) -> list[str]:
    """Task ids whose held-out set differs from heldout.lock (changed, added or never frozen)."""
    locked = json.load(open(HELDOUT_LOCK, encoding="utf-8")) if os.path.exists(HELDOUT_LOCK) else {}
    current = heldout_hashes()
    return [t.id for t in tasks if t.heldout is not None and locked.get(t.id) != current.get(t.id)]


def load_tasks(selected: list[str] | None = None) -> list[Task]:
    tasks = []
    for path in sorted(glob.glob(os.path.join(HARNESS, "tasks", "*", "task.json"))):
        directory = os.path.dirname(path)
        task_id = os.path.basename(directory)
        if selected and task_id not in selected:
            continue
        with open(path, encoding="utf-8") as f:
            meta = json.load(f)
        starter_path = os.path.join(directory, "starter.tes")
        heldout_path = os.path.join(directory, "heldout.json")
        heldout, policy = None, []
        if os.path.exists(heldout_path):
            with open(heldout_path, encoding="utf-8") as f:
                frozen = json.load(f)
            heldout, policy = frozen["tests"], frozen.get("policy", [])
        multi = os.path.isdir(os.path.join(directory, "starter"))
        solution_files = read_tree(os.path.join(directory, "solution")) if multi else None
        tasks.append(Task(
            id=task_id,
            kind=meta["kind"],
            prompt=open(os.path.join(directory, "task.md"), encoding="utf-8").read(),
            tests=meta["tests"],
            effects=meta["effects"],
            solution=solution_files["main.tes"] if multi else open(os.path.join(directory, "solution.tes"), encoding="utf-8").read(),
            starter=None if multi else (open(starter_path, encoding="utf-8").read() if os.path.exists(starter_path) else None),
            suite=meta.get("suite", "core"),
            examples=meta.get("examples", 2),
            heldout=heldout,
            policy=policy,
            starter_files=read_tree(os.path.join(directory, "starter")) if multi else None,
            solution_files=solution_files,
        ))
    return tasks


def task_message(task: Task, variant: Variant) -> str:
    text = "Task:\n\n" + task.prompt
    if task.starter is not None:
        text += "\nThe current program:\n\n```tessera\n" + reindent(task.starter, variant.indent) + "```\n"
    if task.multi_file:
        text += ("\nThe current program has several files; `main.tes` is the entry file and the other files are its modules. "
                 "Unlike the general rule, reply with one ```tessera code block per file you add or change, each starting with "
                 "the line `// file: PATH` (for example `// file: main.tes`) followed by the complete new content of that file. "
                 "Files you do not include stay as they are.\n")
        for path, source in task.starter_files.items():
            text += f"\n```tessera\n// file: {path}\n" + reindent(source, variant.indent) + "```\n"
    examples = task.tests[:task.examples]
    text += "\nExamples (arguments, expected output, expected exit code):\n"
    for test in examples:
        text += f"\n- arguments: {json.dumps(test['args'])}\n  exit code: {test['exit']}\n  output:\n```\n{test['stdout']}```\n"
        if "stderr" in test:
            text += f"  standard error:\n```\n{test['stderr']}```\n"
    return text


# ---------------------------------------------------------------------------
# Compiling and testing an answer
# ---------------------------------------------------------------------------


def extract_code(answer: str) -> str | None:
    match = re.search(r"```tessera[^\n]*\n(.*?)```", answer, re.S) or re.search(r"```[^\n]*\n(.*?)```", answer, re.S)
    return match.group(1) if match else None


FILE_LINE = re.compile(r"\A\s*//\s*file:\s*(\S+)\s*\n")


def extract_files(answer: str) -> dict[str, str] | None:
    """Multi-file answers: every ```tessera block that starts with `// file: PATH`."""
    files = {}
    for block in re.findall(r"```[^\n]*\n(.*?)```", answer, re.S):
        match = FILE_LINE.match(block)
        if match:
            path = match.group(1).strip("`")
            if path.startswith("/") or ".." in path.split("/") or not path.endswith(".tes"):
                continue
            files[path] = block[match.end():]
    return files or None


def main_effects(code: str) -> list[str]:
    """Effects declared by the entry function: `main`, or `handle` for web applications."""
    found: list[str] = []
    for pattern in (r"fn\s+main\s*\(\s*\)\s*->\s*Unit(?:\s+uses\s+([a-z_,\s]+?))?\s*(?:=|\{)",
                    r"fn\s+handle\s*\([^)]*\)\s*->\s*[\w.]+(?:\s+uses\s+([a-z_,\s]+?))?\s*(?:=|\{)"):
        match = re.search(pattern, code)
        if match and match.group(1):
            found += [e.strip() for e in match.group(1).split(",") if e.strip()]
    return found


def web_view(stdout: str) -> str:
    """For web tasks the `request` test host prints status line, headers, a blank line and the body;
    only the status line and the body are compared, so any correct choice of headers passes."""
    head, separator, body = stdout.partition("\n\n")
    return head.split("\n")[0] + "\n\n" + body if separator else stdout


@dataclass
class Check:
    stage: str           # "no_code", "effects", "compile", "test", "pass", "backend"
    feedback: str = ""   # message for the model
    detail: dict = field(default_factory=dict)


def run_test(binary: str, test: dict, task: Task, workdir: str) -> tuple[bool, dict]:
    try:
        run = subprocess.run([binary] + test["args"], cwd=workdir, capture_output=True, text=True, timeout=RUN_TIMEOUT)
        stdout, exit_code, stderr = run.stdout, run.returncode, run.stderr
    except subprocess.TimeoutExpired:
        stdout, exit_code, stderr = "", None, f"(no result: the program ran longer than {RUN_TIMEOUT} seconds)"
    if task.kind == "web":
        stdout = web_view(stdout)
    # Standard error is only compared for tests that specify it (error cases).
    stderr_wrong = "stderr" in test and stderr != test["stderr"]
    ok = stdout == test["stdout"] and exit_code == test["exit"] and not stderr_wrong
    return ok, {"stdout": stdout, "stderr": stderr, "exit": exit_code}


def write_program(files: dict[str, str], workdir: str) -> None:
    for path, source in files.items():
        target = os.path.join(workdir, *path.split("/"))
        os.makedirs(os.path.dirname(target), exist_ok=True)
        # Tessera source is LF, and `newline=""` keeps Windows from turning it into CRLF (which is E0101).
        with open(target, "w", encoding="utf-8", newline="") as f:
            f.write(source)


def build_program(workdir: str) -> subprocess.CompletedProcess:
    return subprocess.run([TESSERA, "build", "main.tes", "-o", "program", "--opt", "0"], cwd=workdir,
                          capture_output=True, text=True, timeout=COMPILE_TIMEOUT)


def binary_path(workdir: str) -> str:
    # The compiler adds the system's extension.
    return os.path.join(workdir, "program.exe" if os.name == "nt" else "program")


def program_files(answer_files: dict[str, str] | None, code: str | None, task: Task, previous: dict[str, str] | None) -> dict[str, str] | None:
    """The complete program an answer stands for. Multi-file answers overlay the files of the
    previous attempt (the starter files at first)."""
    if not task.multi_file:
        return {"main.tes": code} if code is not None else None
    if not answer_files:
        return None
    files = dict(previous or task.starter_files)
    files.update(answer_files)
    return files


def check_answer(files: dict[str, str] | None, task: Task, workdir: str) -> Check:
    if files is None:
        if task.multi_file:
            return Check("no_code", "Your answer contained no ```tessera code block starting with `// file: PATH`. "
                         "Reply with one such block per file you change.")
        return Check("no_code", "Your answer did not contain a ```tessera code block. Reply with the complete program in one ```tessera code block.")
    disallowed = [e for e in main_effects(files.get("main.tes", "")) if e not in task.effects]
    if disallowed:
        return Check("effects", f"`main` declares effects that are not allowed in this task: {', '.join(disallowed)}. "
                     f"Only {', '.join(task.effects)} may be used.", {"effects": disallowed})
    if "main.tes" not in files:
        return Check("no_code", "The program has no `main.tes`.")
    write_program(files, workdir)
    build = build_program(workdir)
    if build.returncode == 1:
        return Check("compile", "The compiler reported:\n\n```\n" + build.stderr.strip() + "\n```",
                     {"diagnostics": build.stderr})
    if build.returncode != 0:
        return Check("backend", "", {"stderr": build.stderr, "exit": build.returncode})
    # Only the example and feedback tests: the held-out set is never run here.
    for number, test in enumerate(task.tests, 1):
        ok, got = run_test(binary_path(workdir), test, task, workdir)
        if not ok:
            stdout, stderr, exit_code = got["stdout"], got["stderr"], got["exit"]
            message = (f"The program compiled, but test {number} of {len(task.tests)} failed.\n\n"
                       f"Arguments: {json.dumps(test['args'])}\n\nExpected output:\n```\n{test['stdout']}```\n"
                       f"Actual output:\n```\n{stdout}```\nExpected exit code {test['exit']}, got {exit_code}.")
            if "stderr" in test:
                message += f"\n\nExpected standard error:\n```\n{test['stderr']}```\nActual standard error:\n```\n{stderr[:2000]}```"
            elif stderr.strip():
                message += "\n\nStandard error:\n```\n" + stderr.strip()[:2000] + "\n```"
            return Check("test", message, {"test": number, "stdout": stdout, "stderr": stderr, "exit": exit_code})
    return Check("pass")


def check_policy(files: dict[str, str], rules: list[dict]) -> list[str]:
    """Frozen source checks: `{"file": F, "contains": S}` or `{"file": F, "absent": S}`."""
    broken = []
    for rule in rules:
        source = files.get(rule["file"], "")
        if "contains" in rule and rule["contains"] not in source:
            broken.append(f"{rule['file']} must contain {rule['contains']!r}")
        if "absent" in rule and rule["absent"] in source:
            broken.append(f"{rule['file']} must not contain {rule['absent']!r}")
    return broken


def score_heldout(files: dict[str, str] | None, task: Task, workdir: str) -> dict:
    """Run the frozen held-out set once on the final program. Nothing of it reaches the model."""
    total = len(task.heldout) + len(task.policy)
    result = {"total": total, "passed": 0, "failed": [], "policy_broken": []}
    if files is None:
        result["failed"] = list(range(1, len(task.heldout) + 1))
        result["policy_broken"] = ["no program"]
        return result
    write_program(files, workdir)
    if build_program(workdir).returncode != 0:
        result["failed"] = list(range(1, len(task.heldout) + 1))
        result["policy_broken"] = check_policy(files, task.policy)
        return result
    for number, test in enumerate(task.heldout, 1):
        ok, _ = run_test(binary_path(workdir), test, task, workdir)
        if ok:
            result["passed"] += 1
        else:
            result["failed"].append(number)
    broken = check_policy(files, task.policy)
    result["policy_broken"] = broken
    result["passed"] += len(task.policy) - len(broken)
    return result


# ---------------------------------------------------------------------------
# Model access
# ---------------------------------------------------------------------------


class Model:
    """Thin wrapper so that dry runs can replace the API."""

    def __init__(self, name: str, effort: str):
        import anthropic  # imported here so that dry runs work without the package
        self.anthropic = anthropic
        self.client = anthropic.Anthropic(max_retries=6)
        self.name = name
        self.effort = effort

    def answer(self, system: str, messages: list[dict]) -> tuple[str, dict]:
        # Streaming: the SDK refuses non-streaming requests with a max_tokens this large.
        with self.client.messages.stream(
            model=self.name,
            max_tokens=32000,
            system=[{"type": "text", "text": system, "cache_control": {"type": "ephemeral"}}],
            messages=messages,
            thinking={"type": "adaptive"},
            output_config={"effort": self.effort},
        ) as stream:
            response = stream.get_final_message()
            request_id = stream.request_id
        text = "".join(block.text for block in response.content if block.type == "text")
        usage = response.usage
        return text, {
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "cache_creation_input_tokens": usage.cache_creation_input_tokens or 0,
            "cache_read_input_tokens": usage.cache_read_input_tokens or 0,
            "stop_reason": response.stop_reason,
            "model": response.model,
            "request_id": request_id,
        }

    def count_tokens(self, text: str) -> int:
        return self.client.messages.count_tokens(model=self.name, messages=[{"role": "user", "content": text}]).input_tokens


class ReferenceModel:
    """Dry run: answers with the task's reference solution, re-indented for the variant."""

    name = "reference-solution"
    effort = "n/a"

    def __init__(self, tasks: dict[str, Task], variant: Variant):
        self.tasks = tasks
        self.variant = variant
        self.current: str | None = None

    def answer(self, system: str, messages: list[dict]) -> tuple[str, dict]:
        task = self.tasks[self.current]
        if task.multi_file:
            text = "".join(f"```tessera\n// file: {path}\n" + reindent(source, self.variant.indent) + "```\n"
                           for path, source in task.solution_files.items())
        else:
            text = "```tessera\n" + reindent(task.solution, self.variant.indent) + "```"
        return text, {"input_tokens": 0, "output_tokens": 0, "cache_creation_input_tokens": 0,
                                                "cache_read_input_tokens": 0, "stop_reason": "end_turn", "model": self.name}

    def count_tokens(self, text: str) -> int:
        return 0


# ---------------------------------------------------------------------------
# Running tasks
# ---------------------------------------------------------------------------


def files_text(files: dict[str, str] | None) -> str | None:
    if not files:
        return None
    return "".join(f"// file: {path}\n{source}" for path, source in sorted(files.items()))


def solve(task: Task, variant: Variant, model, out_dir: str, repetition: int) -> dict:
    system = system_prompt(variant)
    messages: list[dict] = [{"role": "user", "content": task_message(task, variant)}]
    attempts = []
    record = {"task": task.id, "kind": task.kind, "suite": task.suite, "variant": variant.name, "repetition": repetition,
              "model": model.name, "effort": model.effort, "attempts": attempts}
    files: dict[str, str] | None = None
    workdir = tempfile.mkdtemp(prefix=f"tessera-{task.id}-")
    try:
        if isinstance(model, ReferenceModel):
            model.current = task.id
        for number in range(1, MAX_ATTEMPTS + 1):
            text, usage = model.answer(system, messages)
            if usage["stop_reason"] == "refusal":
                attempts.append({"attempt": number, "usage": usage, "stage": "refusal"})
                break
            code = None if task.multi_file else extract_code(text)
            answer_files = extract_files(text) if task.multi_file else None
            candidate = program_files(answer_files, code, task, files)
            if candidate is not None:
                files = candidate
            check = check_answer(candidate, task, workdir)
            attempts.append({"attempt": number, "usage": usage, "stage": check.stage,
                             "code": code if not task.multi_file else answer_files,
                             "answer": text, "detail": check.detail})
            if check.stage in ("pass", "backend"):
                break
            messages.append({"role": "assistant", "content": text})
            messages.append({"role": "user", "content": check.feedback + "\n\nReply with the complete corrected program in one ```tessera code block."})
        final = attempts[-1]
        record["passed"] = final["stage"] == "pass"
        record["first_compiled"] = attempts[0]["stage"] in ("test", "pass")
        record["first_passed"] = attempts[0]["stage"] == "pass"
        record["attempts_used"] = len(attempts)
        record["final_stage"] = final["stage"]
        record["output_tokens"] = sum(a["usage"]["output_tokens"] for a in attempts)
        record["input_tokens"] = sum(a["usage"]["input_tokens"] + a["usage"]["cache_creation_input_tokens"]
                                     + a["usage"]["cache_read_input_tokens"] for a in attempts)
        record["uncached_input_tokens"] = sum(a["usage"]["input_tokens"] for a in attempts)
        final_code = files_text(files) if task.multi_file and final["stage"] != "refusal" else final.get("code")
        record["program_tokens"] = model.count_tokens(final_code) if final_code and record["passed"] else None
        record["program_chars"] = len(final_code) if final_code else None
        if task.multi_file and files is not None:
            record["final_files"] = files
        if task.heldout is not None:
            # Scored on the last program the model gave, whether or not it passed the feedback tests.
            final_files = files if task.multi_file else ({"main.tes": final["code"]} if final.get("code") else None)
            held = score_heldout(final_files if final["stage"] != "refusal" else None, task, workdir)
            record["heldout"] = held
            record["heldout_passed"] = held["passed"] == held["total"]
            record["heldout_fraction"] = held["passed"] / held["total"] if held["total"] else None
    finally:
        shutil.rmtree(workdir, ignore_errors=True)
    path = os.path.join(out_dir, f"{variant.name}--{task.id}--{repetition}.json")
    with open(path, "w", encoding="utf-8") as f:
        json.dump(record, f, indent=2)
    return record


def run(args: argparse.Namespace, dry: bool) -> str:
    if not os.path.exists(TESSERA):
        sys.exit(f"error: {TESSERA} not found; run ./build.sh first")
    variants = load_variants()
    chosen = [variants[name] for name in args.variants]
    tasks = load_tasks(args.tasks)
    unfrozen = check_frozen(tasks)
    if unfrozen and not dry:
        sys.exit("error: the held-out tests of " + ", ".join(unfrozen) + " differ from harness/heldout.lock; "
                 "a held-out set may only change before it is used: run `harness.py freeze` deliberately")
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    out_dir = os.path.join(HARNESS, "results", ("dry-" if dry else "") + stamp)
    os.makedirs(out_dir)
    config = {"variants": args.variants, "tasks": [t.id for t in tasks], "repetitions": args.repeat,
              "model": "reference-solution" if dry else args.model, "effort": None if dry else args.effort,
              "max_attempts": MAX_ATTEMPTS,
              "heldout_lock": file_hash(HELDOUT_LOCK) if os.path.exists(HELDOUT_LOCK) else None,
              "unfrozen": unfrozen}
    with open(os.path.join(out_dir, "config.json"), "w", encoding="utf-8") as f:
        json.dump(config, f, indent=2)
    lock = threading.Lock()
    jobs = []
    for variant in chosen:
        for repetition in range(1, args.repeat + 1):
            for task in tasks:
                jobs.append((task, variant, repetition))

    def work(job):
        task, variant, repetition = job
        model = ReferenceModel({t.id: t for t in tasks}, variant) if dry else Model(args.model, args.effort)
        record = solve(task, variant, model, out_dir, repetition)
        with lock:
            status = "pass" if record["passed"] else record["final_stage"]
            held = record.get("heldout")
            held_text = f", held-out {held['passed']}/{held['total']}" if held else ""
            print(f"{variant.name:24} {task.id:24} rep {repetition}: {status} after {record['attempts_used']} attempt(s){held_text}", flush=True)
        return record

    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        records = list(pool.map(work, jobs))
    summary = summarise(records)
    with open(os.path.join(out_dir, "summary.md"), "w", encoding="utf-8") as f:
        f.write(summary)
    print("\n" + summary)
    print(f"results: {os.path.relpath(out_dir, REPO)}")
    return out_dir


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------


def mean(values: list[float]) -> float | None:
    values = [v for v in values if v is not None]
    return sum(values) / len(values) if values else None


def fmt(value: float | None, digits: int = 0, percent: bool = False) -> str:
    if value is None:
        return "-"
    if percent:
        return f"{value * 100:.0f}%"
    return f"{value:,.{digits}f}"


def summarise(records: list[dict]) -> str:
    groups: dict[tuple[str, str], list[dict]] = {}
    for r in records:
        groups.setdefault((r["variant"], r.get("suite", "core")), []).append(r)
    lines = ["| Variant | Suite | Runs | Passed (feedback tests) | Passed held-out | Held-out tests passed | Passed first try | Compiled first try | Attempts | Output tokens | Input tokens | Program tokens |",
             "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for (name, suite), rs in sorted(groups.items()):
        scored = [r for r in rs if r.get("heldout") is not None]
        lines.append("| " + " | ".join([
            name, suite, str(len(rs)),
            fmt(mean([1.0 if r["passed"] else 0.0 for r in rs]), percent=True),
            fmt(mean([1.0 if r["heldout_passed"] else 0.0 for r in scored]), percent=True) if scored else "-",
            fmt(mean([r["heldout_fraction"] for r in scored]), percent=True) if scored else "-",
            fmt(mean([1.0 if r["first_passed"] else 0.0 for r in rs]), percent=True),
            fmt(mean([1.0 if r["first_compiled"] else 0.0 for r in rs]), percent=True),
            fmt(mean([r["attempts_used"] for r in rs]), 2),
            fmt(mean([r["output_tokens"] for r in rs])),
            fmt(mean([r["input_tokens"] for r in rs])),
            fmt(mean([r["program_tokens"] for r in rs])),
        ]) + " |")
    failures = [r for r in records if not r["passed"]]
    if failures:
        lines.append("\nNot passed (feedback tests): " + ", ".join(f"{r['variant']}/{r['task']}#{r['repetition']} ({r['final_stage']})" for r in sorted(failures, key=lambda r: (r["task"], r["repetition"]))))
    held_failures = [r for r in records if r.get("heldout") is not None and not r["heldout_passed"]]
    if held_failures:
        lines.append("\nNot passed (held-out): " + ", ".join(
            f"{r['variant']}/{r['task']}#{r['repetition']} ({r['heldout']['passed']}/{r['heldout']['total']}"
            + ("; policy" if r["heldout"]["policy_broken"] else "") + ")"
            for r in sorted(held_failures, key=lambda r: (r["task"], r["repetition"]))))
    lines.append("\nAverages are per task run. \"Passed\" uses the example and feedback tests the model sees on failure; "
                 "\"Passed held-out\" is the share of runs whose final program passes every frozen held-out test and policy check, "
                 "which the model never sees. Output tokens include thinking. Program tokens: the final passing program, counted with the API.")
    return "\n".join(lines) + "\n"


def report(args: argparse.Namespace) -> None:
    records = []
    for directory in args.directories:
        for path in glob.glob(os.path.join(directory, "*--*--*.json")):
            with open(path, encoding="utf-8") as f:
                records.append(json.load(f))
    print(summarise(records))


# ---------------------------------------------------------------------------
# Static token counts
# ---------------------------------------------------------------------------


def corpus(name: str) -> list[str]:
    sets = {
        "examples": sorted(glob.glob(os.path.join(REPO, "examples", "**", "*.tes"), recursive=True)),
        "tests": sorted(glob.glob(os.path.join(REPO, "tests", "run", "*.tes")) + glob.glob(os.path.join(REPO, "tests", "stage1_run", "*.tes"))),
        "compiler": sorted(p for p in glob.glob(os.path.join(REPO, "stage1", "*.tes")) if not p.endswith("runtime_text.tes")),
        "solutions": sorted(glob.glob(os.path.join(HARNESS, "tasks", "*", "solution.tes"))),
    }
    return sets[name]


def tokens(args: argparse.Namespace) -> None:
    model = Model(args.model, "medium")
    widths = [0, 2, 4]
    print(f"Tokens counted with {args.model} via the token counting API (a constant message overhead of a few tokens per file is included).\n")
    print("| Corpus | Files | Characters (no indentation) | " + " | ".join(f"Tokens, indent {w}" for w in widths) + " |")
    print("|---|---|---|" + "---|" * len(widths))
    for name in args.corpora:
        files = corpus(name)
        texts = [open(p, encoding="utf-8").read() for p in files]
        counts = [sum(model.count_tokens(reindent(t, w)) for t in texts) for w in widths]
        base = counts[0]
        cells = [f"{c:,} ({(c - base) / base * 100:+.1f}%)" if w != 0 else f"{c:,}" for c, w in zip(counts, widths)]
        print(f"| {name} | {len(files)} | {sum(len(reindent(t, 0)) for t in texts):,} | " + " | ".join(cells) + " |")
    if args.compare:
        print("\n| Directory | Characters | Tokens | Change |\n|---|---|---|---|")
        first = None
        for directory in args.compare:
            paths = [p for p in sorted(glob.glob(os.path.join(REPO, directory, "*.tes"))) if not p.endswith("runtime_text.tes")]
            texts = [open(p, encoding="utf-8").read() for p in paths]
            count = sum(model.count_tokens(t) for t in texts)
            first = first or count
            print(f"| {directory} | {sum(len(t) for t in texts):,} | {count:,} | {(count - first) / first * 100:+.1f}% |")


# ---------------------------------------------------------------------------


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ("run", "dry-run"):
        p = sub.add_parser(name)
        p.add_argument("--variants", nargs="+", default=["baseline"])
        p.add_argument("--tasks", nargs="+", help="task ids (default: all)")
        p.add_argument("--repeat", type=int, default=1)
        p.add_argument("--jobs", type=int, default=4)
        p.add_argument("--model", default=DEFAULT_MODEL)
        p.add_argument("--effort", default="medium", choices=["low", "medium", "high", "xhigh", "max"])
    p = sub.add_parser("tokens")
    p.add_argument("--corpora", nargs="+", default=["examples", "tests", "compiler", "solutions"])
    p.add_argument("--compare", nargs="+", metavar="DIR", help="also count directories of .tes files (relative to the repository), first one is the base")
    p.add_argument("--model", default=DEFAULT_MODEL)
    sub.add_parser("freeze", help="record the current held-out sets in heldout.lock")
    p = sub.add_parser("mutate", help="mutation testing of the reference solutions against the test sets")
    p.add_argument("--tasks", nargs="+")
    p.add_argument("--jobs", type=int, default=8)
    p.add_argument("--limit", type=int, default=0, help="at most this many mutants per task (seeded sample; 0 = all)")
    p = sub.add_parser("report")
    p.add_argument("directories", nargs="+")
    args = parser.parse_args()
    if args.command == "run":
        run(args, dry=False)
    elif args.command == "dry-run":
        run(args, dry=True)
    elif args.command == "tokens":
        tokens(args)
    elif args.command == "freeze":
        hashes = heldout_hashes()
        with open(HELDOUT_LOCK, "w", encoding="utf-8") as f:
            json.dump(hashes, f, indent=2, sort_keys=True)
            f.write("\n")
        print(f"froze the held-out sets of {len(hashes)} tasks in {os.path.relpath(HELDOUT_LOCK, REPO)}")
    elif args.command == "mutate":
        import mutation
        mutation.main(args, sys.modules[__name__])
    else:
        report(args)


if __name__ == "__main__":
    main()
