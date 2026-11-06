"""Writes harness/tasks/<id>/{task.md,task.json,solution.tes[,starter.tes]} (part 1: write-from-scratch tasks)."""
import json, os, sys
ROOT = os.path.join(os.path.dirname(__file__), "..", "tasks")

def task(tid, kind, prompt, tests, solution, starter=None, effects=("io", "env")):
    d = os.path.join(ROOT, tid)
    os.makedirs(d, exist_ok=True)
    open(os.path.join(d, "task.md"), "w").write(prompt.strip() + "\n")
    open(os.path.join(d, "solution.tes"), "w").write(solution.lstrip("\n"))
    if starter:
        open(os.path.join(d, "starter.tes"), "w").write(starter.lstrip("\n"))
    json.dump({"kind": kind, "effects": list(effects), "tests": tests}, open(os.path.join(d, "task.json"), "w"), indent=2)

def t(args, stdout, exit=0):
    return {"args": args, "stdout": stdout, "exit": exit}

task("fizzbuzz", "write", """
Write a program that takes one command-line argument, a positive integer N, and prints the numbers 1 to N, one per line. For multiples of 3 print `Fizz` instead of the number, for multiples of 5 print `Buzz`, and for multiples of both print `FizzBuzz`.
""", [t(["15"], "1\n2\nFizz\n4\nBuzz\nFizz\n7\n8\nFizz\nBuzz\n11\nFizz\n13\n14\nFizzBuzz\n"), t(["3"], "1\n2\nFizz\n")], """
fn parse(text: Text) -> Int {
    var value = 0
    for code in code_points(text) {
        set value = value * 10 + (code - 48)
    }
    value
}

fn line(n: Int) -> Text {
    if n % 15 == 0 {
        "FizzBuzz"
    } else if n % 3 == 0 {
        "Fizz"
    } else if n % 5 == 0 {
        "Buzz"
    } else {
        to_text(n)
    }
}

fn main() -> Unit uses io, env {
    let n = parse(args()[0])
    for i in range(1, n + 1) {
        print(line(i))
    }
}
""")

task("roman", "write", """
Write a program that converts each command-line argument to a Roman numeral and prints the results, one per line, in order. Use the standard subtractive notation (4 is IV, 9 is IX, 40 is XL, 90 is XC, 400 is CD, 900 is CM). An argument that is not a decimal number from 1 to 3999 prints `invalid` instead. If any argument was invalid, exit with code 1 after printing all lines; otherwise exit with code 0.
""", [t(["1", "4", "9", "14", "40", "90", "400", "1994", "3999"], "I\nIV\nIX\nXIV\nXL\nXC\nCD\nMCMXCIV\nMMMCMXCIX\n"),
      t(["58", "0", "abc", "4000", "12"], "LVIII\ninvalid\ninvalid\ninvalid\nXII\n", 1)], """
fn parse(text: Text) -> Option[Int] {
    let codes = code_points(text)
    if length(codes) == 0 or length(codes) > 4 {
        return Option.None
    }
    var value = 0
    for code in codes {
        if code < 48 or code > 57 {
            return Option.None
        }
        set value = value * 10 + (code - 48)
    }
    Option.Some(value)
}

fn roman(number: Int) -> Text {
    let values = [1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1]
    let symbols = ["M", "CM", "D", "CD", "C", "XC", "L", "XL", "X", "IX", "V", "IV", "I"]
    var remaining = number
    var out = ""
    for index in range(0, length(values)) {
        while remaining >= values[index] {
            set out = out + symbols[index]
            set remaining = remaining - values[index]
        }
    }
    out
}

fn main() -> Unit uses io, env {
    var failed = false
    for argument in args() {
        let number = value_or(parse(argument), 0)
        if number >= 1 and number <= 3999 {
            print(roman(number))
        } else {
            print("invalid")
            set failed = true
        }
    }
    if failed {
        exit(1)
    }
}
""")

task("caesar", "write", """
Write a program for a Caesar cipher. The first command-line argument is the shift, an integer that may be negative (for example `3` or `-2`). The remaining arguments are words. Shift every ASCII letter in the words by the shift, wrapping around within `a`-`z` and within `A`-`Z`; keep the case. Other characters stay unchanged. Print the shifted words on one line, separated by single spaces.
""", [t(["3", "Hello,", "World!"], "Khoor, Zruog!\n"), t(["-1", "abc", "XYZ", "a1"], "zab WXY z1\n"), t(["27", "zZ"], "aA\n")], """
fn parse_shift(text: Text) -> Int {
    let codes = code_points(text)
    var sign = 1
    var start = 0
    if length(codes) > 0 and codes[0] == 45 {
        set sign = -1
        set start = 1
    }
    var value = 0
    for index in range(start, length(codes)) {
        set value = value * 10 + (codes[index] - 48)
    }
    sign * value
}

fn rotate(code: Int, base: Int, shift: Int) -> Int {
    let offset = ((code - base + shift) % 26 + 26) % 26
    base + offset
}

fn shift_word(word: Text, shift: Int) -> Text {
    var out: List[Int] = []
    for code in code_points(word) {
        if code >= 97 and code <= 122 {
            set out = append(out, rotate(code, 97, shift))
        } else if code >= 65 and code <= 90 {
            set out = append(out, rotate(code, 65, shift))
        } else {
            set out = append(out, code)
        }
    }
    from_code_points(out)
}

fn main() -> Unit uses io, env {
    let arguments = args()
    let shift = parse_shift(arguments[0])
    var words: List[Text] = []
    for word in slice(arguments, 1, length(arguments)) {
        set words = append(words, shift_word(word, shift))
    }
    print(join(words, " "))
}
""")

task("brackets", "write", """
Write a program that checks each command-line argument for balanced brackets. The bracket pairs are `()`, `[]` and `{}`; all other characters are ignored. For each argument print `balanced` or `unbalanced` on its own line.
""", [t(["([]{})", "(]", "((", "a(b)c", "", "}{"], "balanced\nunbalanced\nunbalanced\nbalanced\nbalanced\nunbalanced\n")], """
fn opening_for(code: Int) -> Int {
    match code {
        41 => 40
        93 => 91
        125 => 123
        _ => 0
    }
}

fn is_balanced(text: Text) -> Bool {
    var stack: List[Int] = []
    for code in code_points(text) {
        if code == 40 or code == 91 or code == 123 {
            set stack = append(stack, code)
        } else if opening_for(code) != 0 {
            let depth = length(stack)
            if depth == 0 or stack[depth - 1] != opening_for(code) {
                return false
            }
            set stack = slice(stack, 0, depth - 1)
        }
    }
    length(stack) == 0
}

fn main() -> Unit uses io, env {
    for argument in args() {
        print(if is_balanced(argument) { "balanced" } else { "unbalanced" })
    }
}
""")

task("run-length", "write", """
Write a run-length encoder and decoder. The program is called with two arguments: a mode (`encode` or `decode`) and a text.
- `encode` replaces every run of the same character by the run length followed by the character: `aaabcc` becomes `3a1b2c`. The input contains no digits.
- `decode` does the reverse: `12x2y` becomes `xxxxxxxxxxxxyy`. Counts can have several digits.
Print the result on one line. An empty text prints an empty line.
""", [t(["encode", "aaabcc"], "3a1b2c\n"), t(["decode", "12x2y"], "xxxxxxxxxxxxyy\n"), t(["encode", "abc"], "1a1b1c\n"), t(["encode", ""], "\n")], """
fn encode(text: Text) -> Text {
    let codes = code_points(text)
    var out = ""
    var index = 0
    while index < length(codes) {
        var run = 1
        while index + run < length(codes) and codes[index + run] == codes[index] {
            set run = run + 1
        }
        set out = out + to_text(run) + from_code_point(codes[index])
        set index = index + run
    }
    out
}

fn decode(text: Text) -> Text {
    var out = ""
    var count = 0
    for code in code_points(text) {
        if code >= 48 and code <= 57 {
            set count = count * 10 + (code - 48)
        } else {
            for step in range(0, count) {
                set out = out + from_code_point(code)
            }
            set count = 0
        }
    }
    out
}

fn main() -> Unit uses io, env {
    let arguments = args()
    let result = if arguments[0] == "encode" { encode(arguments[1]) } else { decode(arguments[1]) }
    print(result)
}
""")

task("primes", "write", """
Write a program that takes one argument N (a non-negative integer) and prints all prime numbers less than or equal to N on one line, separated by single spaces. If there are none, print an empty line.
""", [t(["30"], "2 3 5 7 11 13 17 19 23 29\n"), t(["1"], "\n"), t(["2"], "2\n")], """
fn parse(text: Text) -> Int {
    var value = 0
    for code in code_points(text) {
        set value = value * 10 + (code - 48)
    }
    value
}

fn main() -> Unit uses io, env {
    let limit = parse(args()[0])
    var composite: List[Bool] = []
    for step in range(0, limit + 1) {
        set composite = append(composite, false)
    }
    var primes: List[Text] = []
    for n in range(2, limit + 1) {
        if not composite[n] {
            set primes = append(primes, to_text(n))
            var multiple = n * n
            while multiple <= limit {
                set composite = replace_at(composite, multiple, true)
                set multiple = multiple + n
            }
        }
    }
    print(join(primes, " "))
}
""")

task("word-count", "write", """
Write a program that counts words. Every command-line argument is one word. Words are compared case-insensitively (only ASCII letters change case) and reported in lowercase. Print one line per distinct word, in order of first appearance, formatted as `word: count`.
""", [t(["the", "cat", "The", "hat", "THE", "cat"], "the: 3\ncat: 2\nhat: 1\n"), t(["One"], "one: 1\n")], """
fn lowercase(word: Text) -> Text {
    var out: List[Int] = []
    for code in code_points(word) {
        set out = append(out, if code >= 65 and code <= 90 { code + 32 } else { code })
    }
    from_code_points(out)
}

fn main() -> Unit uses io, env {
    var words: List[Text] = []
    var counts: List[Int] = []
    for argument in args() {
        let word = lowercase(argument)
        var found = false
        for index in range(0, length(words)) {
            if words[index] == word {
                set counts = replace_at(counts, index, counts[index] + 1)
                set found = true
            }
        }
        if not found {
            set words = append(words, word)
            set counts = append(counts, 1)
        }
    }
    for index in range(0, length(words)) {
        print(words[index] + ": " + to_text(counts[index]))
    }
}
""")

task("sort-numbers", "write", """
Write a program that sorts integers. Every command-line argument must be an integer, optionally with a leading `-`. Print the numbers in ascending order on one line, separated by single spaces. If an argument is not an integer, print `error: not a number: ARG` (with the argument) and exit with code 1 without printing anything else.
""", [t(["5", "-3", "10", "0", "-3"], "-3 -3 0 5 10\n"), t(["4", "x1", "2"], "error: not a number: x1\n", 1), t(["7"], "7\n")], """
fn parse(text: Text) -> Result[Int, Text] {
    let codes = code_points(text)
    var start = 0
    var sign = 1
    if length(codes) > 0 and codes[0] == 45 {
        set start = 1
        set sign = -1
    }
    if length(codes) == start {
        return Result.Error(text)
    }
    var value = 0
    for index in range(start, length(codes)) {
        let digit = codes[index] - 48
        if digit < 0 or digit > 9 {
            return Result.Error(text)
        }
        set value = value * 10 + digit
    }
    Result.Ok(sign * value)
}

fn insert_sorted(sorted: List[Int], value: Int) -> List[Int] {
    var index = 0
    while index < length(sorted) and sorted[index] <= value {
        set index = index + 1
    }
    concat(append(slice(sorted, 0, index), value), slice(sorted, index, length(sorted)))
}

fn sort_all(texts: List[Text]) -> Result[List[Int], Text] {
    var sorted: List[Int] = []
    for text in texts {
        set sorted = insert_sorted(sorted, try parse(text))
    }
    Result.Ok(sorted)
}

fn main() -> Unit uses io, env {
    match sort_all(args()) {
        Result.Ok(sorted) => {
            var parts: List[Text] = []
            for value in sorted {
                set parts = append(parts, to_text(value))
            }
            print(join(parts, " "))
        }
        Result.Error(bad) => {
            print("error: not a number: " + bad)
            exit(1)
        }
    }
}
""")
print("part 1 written")
