"""Writes task.md, task.json and heldout.json for the `hard` suite: multi-file maintenance and
policy-enforcement tasks. Their programs live as files in harness/tasks/<id>/starter/ and
harness/tasks/<id>/solution/ (main.tes plus modules).

These tasks were written after the language design they test (draft 0.21) and were not used to
change it. Each task states its new behaviour in the prompt, while some rules the program must keep
are stated only in `note` declarations (and, for policy-pricing, a comment) in the starter code.
The held-out tests and policy checks probe exactly those rules and the edge cases of the new
behaviour. Every expected output here was written by hand from the task text, not produced by
running the reference solutions; `harness.py dry-run` checks that the two agree.
"""
import json
import os

TASKS = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "tasks")


def t(args, stdout, exit=0):
    return {"args": args, "stdout": stdout, "exit": exit}


def e(args, stderr, exit=1, stdout=""):
    return {"args": args, "stdout": stdout, "stderr": stderr, "exit": exit}


def task(tid, prompt, tests, heldout, policy=()):
    d = os.path.join(TASKS, tid)
    assert os.path.isdir(os.path.join(d, "starter")) and os.path.isdir(os.path.join(d, "solution")), tid
    with open(os.path.join(d, "task.md"), "w", encoding="utf-8") as f:
        f.write(prompt.strip() + "\n")
    with open(os.path.join(d, "task.json"), "w", encoding="utf-8") as f:
        json.dump({"kind": "maintain", "suite": "hard", "effects": ["io", "env"], "examples": 2, "tests": tests}, f, indent=2)
        f.write("\n")
    held = {"tests": heldout}
    if policy:
        held["policy"] = list(policy)
    with open(os.path.join(d, "heldout.json"), "w", encoding="utf-8") as f:
        json.dump(held, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"{tid}: {len(tests)} example/feedback tests, {len(heldout)} held-out tests, {len(policy)} policy checks")


# ---------------------------------------------------------------------------

task("maint-access", """
The program below checks access requests against rules. Each command-line argument is a rule (`allow:SUBJECT:ACTION:PATH` or `deny:SUBJECT:ACTION:PATH`) or a check (`check:USER:ACTION:PATH`), and the program prints one line per check. Read the notes in the code: they state policies the program must keep. Extend it with groups:
1. A new argument `member:MEMBER:GROUP` makes MEMBER a member of GROUP. Group names start with `@` and have at least one character after it (for example `@staff`); MEMBER is a user name or a group name. When a group is a member of another group, every member of the first group is also a member of the second. Memberships may form cycles.
2. A rule whose subject is a group applies to every user that is a member of that group, directly or through other groups. A rule whose subject is a user name applies only to that user, also when a group has the same name without the `@`.
3. Memberships, like rules, count wherever they appear among the arguments.
4. A `member` argument whose GROUP is not a group name, and a check whose USER starts with `@`, are invalid: print `error: invalid argument N` to standard error and exit with code 1, as for other invalid arguments.
""", [
    t(["member:ann:@staff", "allow:@staff:read:/docs", "check:ann:read:/docs/a", "check:bob:read:/docs/a"],
      "ann read /docs/a: allowed\nbob read /docs/a: denied\n"),
    e(["member:ann:staff", "check:ann:read:/"], "error: invalid argument 1\n"),
    t(["allow:@all:read:/", "member:@staff:@all", "member:ann:@staff", "check:ann:read:/x"], "ann read /x: allowed\n"),
    t(["member:@a:@b", "member:@b:@a", "member:ann:@a", "allow:@b:write:/w", "check:ann:write:/w", "check:bob:write:/w"],
      "ann write /w: allowed\nbob write /w: denied\n"),
    t(["check:ann:read:/d", "allow:@s:read:/d", "member:ann:@s"], "ann read /d: allowed\n"),
], [
    t(["allow:ann:write:/docs", "member:ann:@interns", "deny:@interns:write:/docs/legal", "check:ann:write:/docs/legal/a", "check:ann:write:/docs/other"],
      "ann write /docs/legal/a: denied\nann write /docs/other: allowed\n"),
    t(["deny:@contractors:*:/finance", "member:@temps:@contractors", "member:joe:@temps", "allow:@temps:read:/", "check:joe:read:/finance/q1", "check:joe:read:/finances"],
      "joe read /finance/q1: denied\njoe read /finances: allowed\n"),
    t(["member:ann:@staff", "allow:staff:read:/x", "allow:@ann:read:/y", "check:ann:read:/x", "check:ann:read:/y", "check:staff:read:/x"],
      "ann read /x: denied\nann read /y: denied\nstaff read /x: allowed\n"),
    t(["member:@a:@b", "member:@b:@c", "member:@c:@a", "member:eve:@c", "allow:@a:read:/", "deny:@b:read:/private", "check:eve:read:/public", "check:eve:read:/private/k"],
      "eve read /public: allowed\neve read /private/k: denied\n"),
    e(["allow:*:read:/", "check:@staff:read:/"], "error: invalid argument 2\n"),
    e(["member:ann:@"], "error: invalid argument 1\n"),
    t(["member:ann:@ops", "allow:*:read:/", "deny:@ops:*:/", "check:ann:read:/a", "check:bob:read:/a"],
      "ann read /a: denied\nbob read /a: allowed\n"),
    t(["member:u:@g1", "member:@g1:@g2", "member:@g2:@g3", "member:@g3:@g4", "allow:@g4:deploy:/prod", "check:u:deploy:/prod", "check:u:deploy:/prod/eu"],
      "u deploy /prod: allowed\nu deploy /prod/eu: allowed\n"),
    t(["member:kim:@dev", "member:kim:@audit", "allow:@dev:write:/src", "deny:@audit:write:/src/main", "check:kim:write:/src/lib", "check:kim:write:/src/main"],
      "kim write /src/lib: allowed\nkim write /src/main: denied\n"),
    e(["check:ann:read:docs"], "error: invalid argument 1\n"),
    e(["allow:*:read:/", "check::read:/"], "error: invalid argument 2\n"),
    e(["member::@s"], "error: invalid argument 1\n"),
])

# ---------------------------------------------------------------------------

task("maint-billing", """
The program below prints an invoice. Each command-line argument is an item `NAME:PRICE:QUANTITY:CATEGORY`; the categories are `food` (9% tax) and `general` (21% tax). Read the notes in the code: they state rules the program must keep. Extend it:
1. Add the categories `books` (9% tax) and `exempt` (0% tax).
2. An argument `discount:P`, where P is a whole number from 0 to 100, gives a discount of P percent on every item line. It may appear anywhere among the arguments, at most once. Each line's net amount (price times quantity, rounded to cents as now) is reduced by P percent and rounded to cents, half away from zero; the line's tax is computed from that discounted amount, and the line shows the discounted amount. A `discount` argument that is malformed, out of range or given a second time is invalid: print `error: invalid argument N` to standard error and exit with code 1, like other invalid arguments.
3. After the `total:` line, print one line `tax R%: T` for every tax rate R that at least one item line uses (including 0%), in ascending order of R, where T is the sum of the line taxes at that rate.
The output is otherwise unchanged.
""", [
    t(["bread:1.95:2:food", "discount:10", "novel:12.50:1:books"],
      "bread 2 x 1.95 = 3.51 tax 0.32\nnovel 1 x 12.50 = 11.25 tax 1.01\nnet: 14.76\ntax: 1.33\ntotal: 16.09\ntax 9%: 1.33\n"),
    t(["gift:5:1:exempt", "pen:2:1:general"],
      "gift 1 x 5.00 = 5.00 tax 0.00\npen 1 x 2.00 = 2.00 tax 0.42\nnet: 7.00\ntax: 0.42\ntotal: 7.42\ntax 0%: 0.00\ntax 21%: 0.42\n"),
    e(["discount:5", "discount:5", "a:1:1:food"], "error: invalid argument 2\n"),
    e(["a:1:1:food", "discount:101"], "error: invalid argument 2\n"),
    t([], "net: 0.00\ntax: 0.00\ntotal: 0.00\n"),
], [
    t(["a:0.07:1:food", "b:0.07:1:food", "c:0.07:1:food"],
      "a 1 x 0.07 = 0.07 tax 0.01\nb 1 x 0.07 = 0.07 tax 0.01\nc 1 x 0.07 = 0.07 tax 0.01\nnet: 0.21\ntax: 0.03\ntotal: 0.24\ntax 9%: 0.03\n"),
    t(["discount:50", "x:0.05:1:exempt", "y:0.25:1:general"],
      "x 1 x 0.05 = 0.03 tax 0.00\ny 1 x 0.25 = 0.13 tax 0.03\nnet: 0.16\ntax: 0.03\ntotal: 0.19\ntax 0%: 0.00\ntax 21%: 0.03\n"),
    t(["z:0.05:3:food", "discount:50"],
      "z 3 x 0.05 = 0.08 tax 0.01\nnet: 0.08\ntax: 0.01\ntotal: 0.09\ntax 9%: 0.01\n"),
    t(["g:10:1:general", "b:10:1:books", "f:10:1:food", "e:1:1:exempt"],
      "g 1 x 10.00 = 10.00 tax 2.10\nb 1 x 10.00 = 10.00 tax 0.90\nf 1 x 10.00 = 10.00 tax 0.90\ne 1 x 1.00 = 1.00 tax 0.00\n"
      "net: 31.00\ntax: 3.90\ntotal: 34.90\ntax 0%: 0.00\ntax 9%: 1.80\ntax 21%: 2.10\n"),
    t(["discount:100", "a:3:2:general"], "a 2 x 3.00 = 0.00 tax 0.00\nnet: 0.00\ntax: 0.00\ntotal: 0.00\ntax 21%: 0.00\n"),
    t(["a:1:1:food", "discount:0"], "a 1 x 1.00 = 1.00 tax 0.09\nnet: 1.00\ntax: 0.09\ntotal: 1.09\ntax 9%: 0.09\n"),
    e(["discount:x"], "error: invalid argument 1\n"),
    e(["a:1:1:food", "discount:-1"], "error: invalid argument 2\n"),
    t(["a:0.07:1:books", "b:0.07:1:food", "discount:0"],
      "a 1 x 0.07 = 0.07 tax 0.01\nb 1 x 0.07 = 0.07 tax 0.01\nnet: 0.14\ntax: 0.02\ntotal: 0.16\ntax 9%: 0.02\n"),
    e(["a:1:0:food"], "error: invalid argument 1\n"),
    e(["b:1:1:food", ":1:1:food"], "error: invalid argument 2\n"),
    e(["a:-1:1:food"], "error: invalid argument 1\n"),
    t(["a:0:1:food"], "a 1 x 0.00 = 0.00 tax 0.00\nnet: 0.00\ntax: 0.00\ntotal: 0.00\ntax 9%: 0.00\n"),
])

# ---------------------------------------------------------------------------

task("maint-schedule", """
The program below manages room bookings. Each command-line argument is a command, run in order: `book:ID:ROOM:SPAN` adds a booking and `cancel:ID` removes one. A SPAN is written `HHMM-HHMM`, for example `0900-1030`. A rejected command prints `rejected N: REASON` (N counts the arguments from 1) and the program goes on; at the end it lists the bookings. Read the notes in the code: they state rules the program must keep. Add two commands:
1. `move:ID:SPAN` gives booking ID a new span in the same room.
2. `swap:ID1:ID2` exchanges the rooms of two bookings; each keeps its own span.
A command that would make two bookings in the same room overlap is rejected with the reason `conflict with X`, where X is the ID of the conflicting booking that starts earliest; for `swap`, the new placement of ID1 is checked before that of ID2. A command naming an unknown booking is rejected with `unknown booking X` (for `swap`, ID1 is checked first). A malformed command, and a `swap` of a booking with itself, is rejected with `invalid`.
""", [
    t(["book:a:red:0900-1000", "book:b:red:1000-1100", "move:a:0800-0900", "move:b:0830-0930"],
      "rejected 4: conflict with a\nred 0800-0900 a\nred 1000-1100 b\n"),
    t(["book:a:red:0900-1000", "book:b:blue:0900-1000", "swap:a:b", "swap:a:c"],
      "rejected 4: unknown booking c\nblue 0900-1000 a\nred 0900-1000 b\n"),
    t(["book:a:red:0900-1000", "move:a:2400-2500", "swap:a:a", "move:a"],
      "rejected 2: invalid\nrejected 3: invalid\nrejected 4: invalid\nred 0900-1000 a\n"),
    t(["book:a:red:0900-1000", "book:b:red:1100-1200", "book:c:blue:1130-1230", "swap:a:c"],
      "rejected 4: conflict with b\nblue 1130-1230 c\nred 0900-1000 a\nred 1100-1200 b\n"),
], [
    t(["book:a:red:0900-1000", "move:a:0930-1030"], "red 0930-1030 a\n"),
    t(["book:a:red:0900-1000", "book:b:blue:0930-1030", "swap:a:b"], "blue 0900-1000 a\nred 0930-1030 b\n"),
    t(["book:a:red:0900-1000", "book:b:red:1100-1200", "move:b:1000-1100", "move:a:1100-1130"], "red 1000-1100 b\nred 1100-1130 a\n"),
    t(["book:z:red:1000-1100", "book:y:red:0800-0900", "book:w:red:1200-1300", "move:w:0830-1030"],
      "rejected 4: conflict with y\nred 0800-0900 y\nred 1000-1100 z\nred 1200-1300 w\n"),
    t(["book:a:red:0900-1000", "book:b:blue:1300-1400", "book:c:blue:0930-0945", "book:d:red:1330-1430", "swap:a:b"],
      "rejected 5: conflict with c\nblue 0930-0945 c\nblue 1300-1400 b\nred 0900-1000 a\nred 1330-1430 d\n"),
    t(["swap:p:q", "book:p:r:0100-0200", "swap:p:q", "move:q:0100-0200"],
      "rejected 1: unknown booking p\nrejected 3: unknown booking q\nrejected 4: unknown booking q\nr 0100-0200 p\n"),
    t(["book:a:red:0900-1000", "move:a:1000-1100", "book:b:red:0900-1000", "cancel:a", "move:b:0930-1100"], "red 0930-1100 b\n"),
    t(["book:a:red:0900-1000", "move:a:1000-0900", "move:a:0900-1000:x", "swap:a"],
      "rejected 2: invalid\nrejected 3: invalid\nrejected 4: invalid\nred 0900-1000 a\n"),
    t(["book:a:red:0900-1000", "book:b:red:1000-1100", "swap:a:b"], "red 0900-1000 a\nred 1000-1100 b\n"),
    t(["book:b:red:1640-1700", "book:a:red:0900-1000", "book:c:red:2300-2359"], "red 0900-1000 a\nred 1640-1700 b\nred 2300-2359 c\n"),
    t(["book:a:red:0000-0001", "book:b:red:2400-2401", "book:c:red:0960-1000", "book:d::0100-0200", "cancel:", "book:e:red:1000-1000", "move:a:0001-0001", "book::red:0500-0600"],
      "rejected 2: invalid\nrejected 3: invalid\nrejected 4: invalid\nrejected 5: invalid\nrejected 6: invalid\nrejected 7: invalid\nrejected 8: invalid\nred 0000-0001 a\n"),
])

# ---------------------------------------------------------------------------

task("policy-pricing", """
The program below prints price quotes. Each command-line argument is `SKU:QUANTITY:CUSTOMER`, and the program prints one line per quote with its total. Read the notes and comments in the code: they state policies the program must keep. Extend it:
1. Add a bulk discount: when the quantity is 10 or more, a further 5% comes off the amount after the member discount, truncated to whole cents like the member discount.
2. When the first argument is `--explain`, print after each quote line the price steps that applied, each on its own line indented by two spaces: `  base: Q x UNIT = AMOUNT`, then `  member discount: -AMOUNT` if the member discount applied, then `  bulk discount: -AMOUNT` if the bulk discount applied. A discount that applied is shown even when it is 0.00. Amounts are written like the totals. Argument numbers in error messages still count `--explain` as argument 1.
Without `--explain` the output format is unchanged.
""", [
    t(["--explain", "pen:10:member"], "pen 10 member: 21.38\n  base: 10 x 2.50 = 25.00\n  member discount: -2.50\n  bulk discount: -1.12\n"),
    t(["ink:12:retail", "pad:1:member"], "ink 12 retail: 148.09\npad 1 member: 4.32\n"),
    t(["--explain", "pad:2:retail"], "pad 2 retail: 9.60\n  base: 2 x 4.80 = 9.60\n"),
    e(["--explain", "pen:0:retail"], "error: invalid argument 2\n"),
], [
    t(["--explain", "clip:1:member"], "clip 1 member: 0.05\n  base: 1 x 0.05 = 0.05\n  member discount: -0.00\n"),
    t(["clip:10:retail"], "clip 10 retail: 0.48\n"),
    t(["--explain", "pen:9:member"], "pen 9 member: 20.25\n  base: 9 x 2.50 = 22.50\n  member discount: -2.25\n"),
    t(["--explain", "ink:11:member"], "ink 11 member: 122.18\n  base: 11 x 12.99 = 142.89\n  member discount: -14.28\n  bulk discount: -6.43\n"),
    e(["pen:1:retail", "--explain"], "error: invalid argument 2\n", stdout="pen 1 retail: 2.50\n"),
    t(["--explain"], ""),
    t(["clip:20:member"], "clip 20 member: 0.86\n"),
    t([], ""),
    e(["pen:x:retail"], "error: invalid argument 1\n"),
    e(["pen:1:retail", "zz:1:retail"], "error: unknown product zz\n", stdout="pen 1 retail: 2.50\n"),
], policy=[
    {"file": "main.tes", "contains": "import pricing uses nothing"},
    {"file": "pricing.tes", "absent": " uses "},
])
