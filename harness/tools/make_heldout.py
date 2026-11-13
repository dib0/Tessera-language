"""Writes harness/tasks/<id>/heldout.json for the core tasks: the frozen held-out test set.

The expected results here are written by hand or computed by small Python oracles below, never by
running the reference solutions, so a reference solution can disagree with them (then one of the two
is wrong, and that is checked with `harness.py dry-run`). Held-out tests only probe behaviour that the
task text states; they are never shown to the model and never fed back to it.

Once frozen (`harness.py freeze`), a held-out set must not change: `harness.py run` refuses to start
when a set differs from harness/heldout.lock. Writing this file again with other tests and freezing
again makes the earlier scores incomparable, so do that only before a set is used.
"""
import json
import os
import sys

TASKS = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "tasks")


def t(args, stdout, exit=0):
    return {"args": args, "stdout": stdout, "exit": exit}


def e(args, stderr, exit=1):
    return {"args": args, "stdout": "", "stderr": stderr, "exit": exit}


def lines(*items):
    return "".join(f"{i}\n" for i in items)


def fizz(n):
    return lines(*["FizzBuzz" if i % 15 == 0 else "Fizz" if i % 3 == 0 else "Buzz" if i % 5 == 0 else i for i in range(1, n + 1)])


def primes(n):
    return " ".join(str(p) for p in range(2, n + 1) if all(p % d for d in range(2, int(p ** 0.5) + 1))) + "\n"


def roman(n):
    out = ""
    for value, sym in [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"),
                       (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")]:
        while n >= value:
            out += sym
            n -= value
    return out


def web(status, body):
    reasons = {200: "OK", 400: "Bad Request", 404: "Not Found", 405: "Method Not Allowed"}
    return f"HTTP/1.1 {status} {reasons[status]}\n\n{body}"


HELDOUT = {
    "anagrams": [
        t(["ab", "ba", "ab"], "ab ba ab\n"),
        t(["abc", "abd", "cab", "bad"], "abc cab\nabd bad\n"),
        t(["aab", "abb", "bab", "aba"], "aab aba\nabb bab\n"),
        t(["x"], "x\n"),
        t([], ""),
    ],
    "brackets": [
        t(["([)]", "{[()()]}", "((()))", "]", "[", "a{b[c]d}e"], lines("unbalanced", "balanced", "balanced", "unbalanced", "unbalanced", "balanced")),
        t(["(" * 10 + ")" * 10, "())(", "{]"], lines("balanced", "unbalanced", "unbalanced")),
        t([], ""),
    ],
    "bst": [
        t(["2", "1", "3", "0", "-1"], "-1 0 1 2 3\nheight: 4\n"),
        t(["7", "7", "7"], "7\nheight: 1\n"),
        t(["4", "2", "6", "1", "3", "5", "7"], "1 2 3 4 5 6 7\nheight: 3\n"),
        t(["10", "5", "15", "5", "12", "20", "25", "30"], "5 10 12 15 20 25 30\nheight: 5\n"),
    ],
    "caesar": [
        t(["0", "Abc-Z"], "Abc-Z\n"),
        t(["26", "Hello"], "Hello\n"),
        t(["-27", "aA"], "zZ\n"),
        t(["13", "Uryyb", "Jbeyq"], "Hello World\n"),
        t(["-100", "a"], "e\n"),
        t(["1", "é", "z"], "é a\n"),
        t(["1", "`{@[", "aZ"], "`{@[ bA\n"),
    ],
    "extend-inventory": [
        t(["add:a:1", "add:b:2", "remove:b:2", "add:c:3"], "a 1\nc 3\n"),
        t(["add:a:1", "add:b:1", "remove:a:1", "add:a:1"], "a 1\nb 1\n"),
        t(["add:a:2", "clear:zz", "add:a:4"], "a 6\n"),
        e(["add:x:1", "clear:x", "remove:x:1"], "error: cannot remove 1 x\n"),
        e(["add:a:3", "bogus"], "error: unknown command bogus\n"),
        t(["add:a:3", "remove:a:3"], ""),
        e(["add:a:1", "remove:a"], "error: unknown command remove:a\n"),
    ],
    "extend-shapes": [
        t(["square:1", "circle:1", "rect:3x4", "rect:1x1"], "1\n3\n12\n1\ntotal: 17\n"),
        t(["rect:12x10", "rect:1x100"], "120\n100\ntotal: 220\n"),
        t(["circle:10"], "300\ntotal: 300\n"),
    ],
    "fix-compile": [
        t(["0", "0"], "average: 0\n"),
        t(["1", "2"], "average: 1\n"),
        t(["7", "8", "9", "10"], "average: 8\n"),
        t(["100", "1"], "average: 50\n"),
    ],
    "fix-logic": [
        t(["xyz"], "xyz: 0\ntotal: 0\n"),
        t(["Queue", "IOU", "rhythm"], "Queue: 4\nIOU: 3\nrhythm: 0\ntotal: 7\n"),
        t([], "total: 0\n"),
        t(["aAaA"], "aAaA: 4\ntotal: 4\n"),
    ],
    "fizzbuzz": [t(["1"], fizz(1)), t(["5"], fizz(5)), t(["31"], fizz(31))],
    "json-total": [
        t(['[{"name": "a", "price": 0.005, "quantity": 1}]'], "items: 1\ntotal: 0.01\n"),
        t(['[{"name": "a", "price": 2, "quantity": 3}]'], "items: 1\ntotal: 6.00\n"),
        e(['[{"name": "a", "price": 1.5, "quantity": 2}, {"name": "b", "price": 1}]'], "error: invalid item 2\n"),
        e(['[5]'], "error: invalid item 1\n"),
        e(['[{"name": 3, "price": 1, "quantity": 1}]'], "error: invalid item 1\n"),
        e(["not json"], "error: invalid JSON\n"),
        e(['"text"'], "error: invalid JSON\n"),
        t(['[{"name": "a", "price": 0.1, "quantity": 3}, {"name": "b", "price": 0.2, "quantity": 1}]'], "items: 2\ntotal: 0.50\n"),
        e(['[{"name": "a", "quantity": 1}]'], "error: invalid item 1\n"),
    ],
    "ledger": [
        t(["deposit:5", "withdraw:5"], "balance: 0\n"),
        e(["withdraw:1"], "error: insufficient funds at transaction 1\n"),
        e(["deposit:0"], "error: invalid transaction at 1\n"),
        e(["deposit:10", "deposit"], "error: invalid transaction at 2\n"),
        e(["deposit:-5"], "error: invalid transaction at 1\n"),
        e(["deposit:10", "withdraw:20", "steal"], "error: insufficient funds at transaction 2\n"),
        t(["deposit:7", "withdraw:2", "withdraw:5", "deposit:1"], "balance: 1\n"),
        e(["deposit:/"], "error: invalid transaction at 1\n"),
        t(["deposit:9", "deposit:19"], "balance: 28\n"),
    ],
    "primes": [t(["0"], "\n"), t(["3"], primes(3)), t(["100"], primes(100)), t(["97"], primes(97))],
    "roman": [
        t(["3888", "2024", "444", "999"], lines(*(roman(n) for n in (3888, 2024, 444, 999)))),
        t(["-5", "1"], "invalid\nI\n", 1),
        t(["1", "3999", "4"], lines("I", "MMMCMXCIX", "IV")),
        t(["40", "49", "90", "99"], lines("XL", "XLIX", "XC", "XCIX")),
        t(["1/", "9"], "invalid\nIX\n", 1),
        t([], ""),
    ],
    "rpn": [
        t(["5"], "5\n"),
        t(["7", "2", "/"], "3\n"),
        t(["2", "7", "-"], "-5\n"),
        t(["0", "7", "-", "2", "/"], "-3\n"),
        e(["-7", "2", "/"], "error: invalid token -7\n"),
        e([], "error: expected one result\n"),
        e(["1", "2", "+", "+"], "error: stack underflow\n"),
        e(["1", "0", "0", "/", "+"], "error: division by zero\n"),
        t(["12", "30", "+"], "42\n"),
        t(["99", "9", "/"], "11\n"),
    ],
    "run-length": [
        t(["encode", "a"], "1a\n"),
        t(["encode", "a" * 12], "12a\n"),
        t(["decode", "1a"], "a\n"),
        t(["decode", ""], "\n"),
        t(["decode", "3a10b"], "aaa" + "b" * 10 + "\n"),
        t(["encode", "aabba"], "2a2b1a\n"),
        t(["encode", "  x"], "2 1x\n"),
        t(["encode", "ééa"], "2é1a\n"),
        t(["decode", "9a"], "aaaaaaaaa\n"),
    ],
    "sort-numbers": [
        t(["-1", "-2", "-10"], "-10 -2 -1\n"),
        e(["2", "--1"], "error: not a number: --1\n"),
        t(["10", "9", "100"], "9 10 100\n"),
        e(["1", "", "2"], "error: not a number: \n"),
        e(["1", "2.5"], "error: not a number: 2.5\n"),
        e(["1:"], "error: not a number: 1:\n"),
        e(["-"], "error: not a number: -\n"),
        t(["9", "-9", "19"], "-9 9 19\n"),
    ],
    "transpose": [
        t(["a,b,c"], "a\nb\nc\n"),
        t(["1,2", "3,4", "5,6"], "1,3,5\n2,4,6\n"),
        t(["a,,b", ",x,"], "a,\n,x\nb,\n"),
        e(["1,2", "3,4,5"], "error: ragged matrix\n"),
        e(["1,2,3", "4,5,6", "7,8"], "error: ragged matrix\n"),
    ],
    "web-router": [
        t(["request", "POST", "/hello"], web(405, "method not allowed")),
        t(["request", "DELETE", "/items/1"], web(405, "method not allowed")),
        t(["request", "GET", "/items/12/extra"], web(404, "not found")),
        t(["request", "GET", "/hello?other=1"], web(200, "hello world")),
        t(["request", "GET", "/hello?name=Bob&x=1"], web(200, "hello Bob")),
        t(["request", "GET", "/helloo"], web(404, "not found")),
        t(["request", "GET", "/items/123"], web(200, "item 123")),
        t(["request", "GET", "/items/1a"], web(400, "bad id")),
    ],
    "word-count": [
        t([], ""),
        t(["Apple", "APPLE", "banana", "apple", "Banana"], "apple: 3\nbanana: 2\n"),
        t(["ÉCOLE", "école"], "école: 2\n"),
        t(["a", "b", "c", "b"], "a: 1\nb: 2\nc: 1\n"),
    ],
    "word-frequency": [
        t(["a", "b", "c", "d"], "a 1\nb 1\nc 1\n"),
        t(["B", "a", "b", "A"], "A 1\nB 1\na 1\n"),
        t(["z", "z", "y", "y", "x", "x", "w", "w", "w"], "w 3\nx 2\ny 2\n"),
        t(["aa", "a", "aa", "a"], "a 2\naa 2\n"),
    ],
    "wrap": [
        t(["5", "hello", "world"], "hello\nworld\n"),
        t(["11", "hello", "world"], "hello world\n"),
        t(["1", "a", "b"], "a\nb\n"),
        t(["4", "abcdefg"], "abcdefg\n"),
        t(["6", "ab", "cd", "efghijk", "l"], "ab cd\nefghijk\nl\n"),
        t(["7", "abc", "abc", "a"], "abc abc\na\n"),
        t(["10", "aaaaa", "bbbbb"], "aaaaa\nbbbbb\n"),
        t(["4", "ab", "cd"], "ab\ncd\n"),
    ],
}


def main():
    selected = sys.argv[1:]
    for task_id, tests in HELDOUT.items():
        if selected and task_id not in selected:
            continue
        path = os.path.join(TASKS, task_id, "heldout.json")
        with open(path, "w", encoding="utf-8") as f:
            json.dump({"tests": tests}, f, indent=2, ensure_ascii=False)
            f.write("\n")
        print(f"{task_id}: {len(tests)} held-out tests")


if __name__ == "__main__":
    main()
