"""Part 2: tasks with records, enums, Result, recursion; fix and extend tasks."""
import os, sys
sys.path.insert(0, os.path.dirname(__file__))
from make_tasks_part1 import task, t, e  # noqa: E402  (re-running part 1 is harmless)

task("ledger", "write", """
Write a program that processes bank transactions given as command-line arguments, in order. The balance starts at 0. Each argument is `deposit:AMOUNT` or `withdraw:AMOUNT`, where AMOUNT is a positive decimal integer.
- After all transactions, print `balance: B`.
- If a withdrawal is larger than the current balance, print `error: insufficient funds at transaction N` (N counts from 1) to standard error and exit with code 1 immediately.
- If an argument is malformed (unknown kind, missing or non-numeric amount, amount 0), print `error: invalid transaction at N` to standard error and exit with code 1 immediately.
""", [t(["deposit:100", "withdraw:30", "deposit:5"], "balance: 75\n"),
      e(["deposit:10", "withdraw:11"], "error: insufficient funds at transaction 2\n"),
      e(["deposit:10", "steal:5"], "error: invalid transaction at 2\n"),
      e(["deposit:x"], "error: invalid transaction at 1\n"),
      t([], "balance: 0\n")], """
enum Kind {
    Deposit
    Withdraw
}

type Transaction {
    kind: Kind
    amount: Int
}

fn split_colon(text: Text) -> List[Text] {
    let codes = code_points(text)
    for index in range(0, length(codes)) {
        if codes[index] == 58 {
            return [from_code_points(slice(codes, 0, index)), from_code_points(slice(codes, index + 1, length(codes)))]
        }
    }
    [text]
}

fn parse_amount(text: Text) -> Option[Int] {
    let codes = code_points(text)
    if length(codes) == 0 {
        return Option.None
    }
    var value = 0
    for code in codes {
        if code < 48 or code > 57 {
            return Option.None
        }
        set value = value * 10 + (code - 48)
    }
    if value == 0 { Option.None } else { Option.Some(value) }
}

fn parse(text: Text) -> Option[Transaction] {
    let parts = split_colon(text)
    if length(parts) != 2 {
        return Option.None
    }
    let amount = try parse_amount(parts[1])
    match parts[0] {
        "deposit" => Option.Some(Transaction { kind: Kind.Deposit, amount: amount })
        "withdraw" => Option.Some(Transaction { kind: Kind.Withdraw, amount: amount })
        _ => Option.None
    }
}

fn main() -> Unit uses io, env {
    var balance = 0
    let arguments = args()
    for index in range(0, length(arguments)) {
        let number = to_text(index + 1)
        match parse(arguments[index]) {
            Option.None => {
                exit_with(1, "error: invalid transaction at " + number)
            }
            Option.Some(transaction) => match transaction.kind {
                Kind.Deposit => {
                    set balance = balance + transaction.amount
                }
                Kind.Withdraw => {
                    if transaction.amount > balance {
                        exit_with(1, "error: insufficient funds at transaction " + number)
                    }
                    set balance = balance - transaction.amount
                }
            }
        }
    }
    print("balance: " + to_text(balance))
}
""")

task("rpn", "write", """
Write a calculator for expressions in reverse Polish notation. Each command-line argument is one token: a non-negative decimal integer, or one of the operators `+`, `-`, `*`, `/` (integer division, truncating toward zero). Evaluate the expression and print the single resulting value.
Errors print one line to standard error and exit with code 1:
- `error: stack underflow` when an operator needs more values than available,
- `error: division by zero`,
- `error: invalid token TOKEN` for anything else,
- `error: expected one result` when the expression leaves zero or several values.
""", [t(["3", "4", "+", "2", "*"], "14\n"), t(["7", "2", "-", "10", "/"], "0\n"), e(["1", "+"], "error: stack underflow\n"),
      e(["4", "0", "/"], "error: division by zero\n"), e(["2", "x", "+"], "error: invalid token x\n"), e(["1", "2"], "error: expected one result\n")], """
fn parse_number(text: Text) -> Option[Int] {
    let codes = code_points(text)
    if length(codes) == 0 {
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

fn apply(operator: Text, a: Int, b: Int) -> Result[Int, Text] {
    match operator {
        "+" => Result.Ok(a + b)
        "-" => Result.Ok(a - b)
        "*" => Result.Ok(a * b)
        _ => if b == 0 { Result.Error("division by zero") } else { Result.Ok(a / b) }
    }
}

fn evaluate(tokens: List[Text]) -> Result[Int, Text] {
    var stack: List[Int] = []
    for token in tokens {
        if token == "+" or token == "-" or token == "*" or token == "/" {
            let depth = length(stack)
            if depth < 2 {
                return Result.Error("stack underflow")
            }
            let value = try apply(token, stack[depth - 2], stack[depth - 1])
            set stack = append(slice(stack, 0, depth - 2), value)
        } else {
            match parse_number(token) {
                Option.Some(number) => {
                    set stack = append(stack, number)
                }
                Option.None => return Result.Error("invalid token " + token)
            }
        }
    }
    if length(stack) != 1 {
        return Result.Error("expected one result")
    }
    Result.Ok(stack[0])
}

fn main() -> Unit uses io, env {
    match evaluate(args()) {
        Result.Ok(value) => print(to_text(value))
        Result.Error(message) => {
            exit_with(1, "error: " + message)
        }
    }
}
""")

task("bst", "write", """
Write a program that inserts the integers given as command-line arguments (possibly negative, written with a leading `-`) into a binary search tree, in order, ignoring duplicates. Then print two lines: the values in ascending order separated by single spaces (an in-order traversal), and `height: H`, where an empty tree has height 0 and a single node has height 1. Use a recursive enum for the tree.
""", [t(["5", "3", "8", "1", "4", "8"], "1 3 4 5 8\nheight: 3\n"), t(["1", "2", "3"], "1 2 3\nheight: 3\n"), t([], "\nheight: 0\n"), t(["-2", "7"], "-2 7\nheight: 2\n")], """
enum Tree {
    Leaf
    Node(left: Tree, value: Int, right: Tree)
}

fn parse(text: Text) -> Int {
    let codes = code_points(text)
    var sign = 1
    var start = 0
    if codes[0] == 45 {
        set sign = -1
        set start = 1
    }
    var value = 0
    for index in range(start, length(codes)) {
        set value = value * 10 + (codes[index] - 48)
    }
    sign * value
}

fn insert(tree: Tree, value: Int) -> Tree {
    match tree {
        Tree.Leaf => Tree.Node(Tree.Leaf, value, Tree.Leaf)
        Tree.Node(left, current, right) => {
            if value < current {
                Tree.Node(insert(left, value), current, right)
            } else if value > current {
                Tree.Node(left, current, insert(right, value))
            } else {
                tree
            }
        }
    }
}

fn in_order(tree: Tree) -> List[Text] {
    match tree {
        Tree.Leaf => []
        Tree.Node(left, value, right) => concat(append(in_order(left), to_text(value)), in_order(right))
    }
}

fn height(tree: Tree) -> Int {
    match tree {
        Tree.Leaf => 0
        Tree.Node(left, _, right) => {
            let a = height(left)
            let b = height(right)
            1 + (if a > b { a } else { b })
        }
    }
}

fn main() -> Unit uses io, env {
    var tree = Tree.Leaf
    for argument in args() {
        set tree = insert(tree, parse(argument))
    }
    print(join(in_order(tree), " "))
    print("height: " + to_text(height(tree)))
}
""")

task("wrap", "write", """
Write a program that word-wraps text. The first command-line argument is the maximum line width W (a positive integer). The remaining arguments are the words, in order. Fill lines greedily: put as many words on a line as fit, separated by single spaces, so that the line is at most W characters long. A word longer than W goes on a line by itself. Print the lines.
""", [t(["10", "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog"], "the quick\nbrown fox\njumps over\nthe lazy\ndog\n"),
      t(["3", "a", "abcdef", "b", "c"], "a\nabcdef\nb c\n")], """
fn parse(text: Text) -> Int {
    var value = 0
    for code in code_points(text) {
        set value = value * 10 + (code - 48)
    }
    value
}

fn main() -> Unit uses io, env {
    let arguments = args()
    let width = parse(arguments[0])
    var line = ""
    for word in slice(arguments, 1, length(arguments)) {
        if line == "" {
            set line = word
        } else if length(line) + 1 + length(word) <= width {
            set line = line + " " + word
        } else {
            print(line)
            set line = word
        }
    }
    if line != "" {
        print(line)
    }
}
""")

task("transpose", "write", """
Write a program that transposes a matrix. Each command-line argument is one row: cells separated by commas, for example `1,2,3`. Cells are arbitrary text without commas. Print the transposed matrix, one row per line, cells separated by commas. If the rows do not all have the same number of cells, print `error: ragged matrix` to standard error and exit with code 1.
""", [t(["1,2,3", "4,5,6"], "1,4\n2,5\n3,6\n"), e(["a,b", "c"], "error: ragged matrix\n"), t(["x"], "x\n")], """
fn cells(row: Text) -> List[Text] {
    var out: List[Text] = []
    var current: List[Int] = []
    for code in code_points(row) {
        if code == 44 {
            set out = append(out, from_code_points(current))
            set current = []
        } else {
            set current = append(current, code)
        }
    }
    append(out, from_code_points(current))
}

fn main() -> Unit uses io, env {
    var rows: List[List[Text]] = []
    for argument in args() {
        set rows = append(rows, cells(argument))
    }
    let width = length(rows[0])
    for row in rows {
        if length(row) != width {
            exit_with(1, "error: ragged matrix")
        }
    }
    for column in range(0, width) {
        var parts: List[Text] = []
        for row in rows {
            set parts = append(parts, row[column])
        }
        print(join(parts, ","))
    }
}
""")

task("anagrams", "write", """
Write a program that groups anagrams. Every command-line argument is a lowercase word. Two words are anagrams if they contain the same letters with the same counts. Print one line per group, in order of the first appearance of the group; within a line, print the group's words in order of appearance, separated by single spaces. Repeated words are kept.
""", [t(["listen", "google", "silent", "enlist", "elgoog", "cat", "act", "tac"], "listen silent enlist\ngoogle elgoog\ncat act tac\n"), t(["a", "b", "a"], "a a\nb\n")], """
fn signature(word: Text) -> List[Int] {
    var counts: List[Int] = []
    for step in range(0, 26) {
        set counts = append(counts, 0)
    }
    for code in code_points(word) {
        set counts = replace_at(counts, code - 97, counts[code - 97] + 1)
    }
    counts
}

fn main() -> Unit uses io, env {
    var keys: List[List[Int]] = []
    var groups: List[List[Text]] = []
    for word in args() {
        let key = signature(word)
        var placed = false
        for index in range(0, length(keys)) {
            if not placed and keys[index] == key {
                set groups = replace_at(groups, index, append(groups[index], word))
                set placed = true
            }
        }
        if not placed {
            set keys = append(keys, key)
            set groups = append(groups, [word])
        }
    }
    for group in groups {
        print(join(group, " "))
    }
}
""")

task("fix-compile", "fix", """
The program below should print the average of its integer command-line arguments, truncated toward zero, as `average: A`, or `no numbers` when there are no arguments. Arguments are non-negative decimal integers. The program does not compile. Fix it so that it compiles and behaves as described; keep its structure.
""", [t(["1", "2", "4"], "average: 2\n"), t([], "no numbers\n"), t(["10"], "average: 10\n")], """
fn parse(text: Text) -> Int {
    var value = 0
    for code in code_points(text) {
        set value = value * 10 + (code - 48)
    }
    value
}

fn report(numbers: List[Int]) -> Unit uses io {
    if length(numbers) == 0 {
        print("no numbers")
        return
    }
    var total = 0
    for number in numbers {
        set total = total + number
    }
    print("average: " + to_text(total / length(numbers)))
}

fn main() -> Unit uses io, env {
    var numbers: List[Int] = []
    for argument in args() {
        set numbers = append(numbers, parse(argument))
    }
    report(numbers)
}
""", starter="""
fn parse(text: Text) -> Int {
    let value = 0
    for code in code_points(text) {
        value = value * 10 + (code - 48)
    }
    value
}

fn report(numbers: List[Int]) -> Unit {
    if length(numbers) == 0 {
        print("no numbers")
        return
    }
    var total = 0
    for number in numbers {
        let total = total + number
    }
    print("average: " + total / length(numbers))
}

fn main() -> Unit uses io, env {
    var numbers = []
    for argument in args() {
        append(numbers, parse(argument))
    }
    report(numbers)
}
""")

task("fix-logic", "fix", """
The program below compiles but is wrong. For each command-line argument it should print the word, a colon, a space and the number of vowels in it (`a`, `e`, `i`, `o`, `u`, upper or lower case), for example `Banana: 3`. At the end it should print `total: T` with the total number of vowels. Find and fix the bugs.
""", [t(["Banana", "sky", "AEIOU"], "Banana: 3\nsky: 0\nAEIOU: 5\ntotal: 8\n"), t(["a"], "a: 1\ntotal: 1\n")], """
fn is_vowel(code: Int) -> Bool {
    let lower = if code >= 65 and code <= 90 { code + 32 } else { code }
    lower == 97 or lower == 101 or lower == 105 or lower == 111 or lower == 117
}

fn count_vowels(word: Text) -> Int {
    let codes = code_points(word)
    var count = 0
    for index in range(0, length(codes)) {
        if is_vowel(codes[index]) {
            set count = count + 1
        }
    }
    count
}

fn main() -> Unit uses io, env {
    var total = 0
    for word in args() {
        let count = count_vowels(word)
        set total = total + count
        print(word + ": " + to_text(count))
    }
    print("total: " + to_text(total))
}
""", starter="""
fn is_vowel(code: Int) -> Bool {
    let lower = if code >= 65 and code <= 90 { code + 32 } else { code }
    lower == 97 or lower == 101 or lower == 105 or lower == 111
}

fn count_vowels(word: Text) -> Int {
    let codes = code_points(word)
    var count = 0
    for index in range(1, length(codes)) {
        if is_vowel(codes[index]) {
            set count = count + 1
        }
    }
    count
}

fn main() -> Unit uses io, env {
    var total = 0
    for word in args() {
        let count = count_vowels(word)
        set total = count
        print(word + ":" + to_text(count))
    }
    print("total: " + to_text(total))
}
""")

task("extend-shapes", "extend", """
The program below reads shapes from its command-line arguments (`circle:R` and `square:S`) and prints the area of each, one per line (circles use 3 as the value of pi). Extend it:
1. Add rectangles, written `rect:WxH` (for example `rect:2x5`, area 10), as a new variant of the `Shape` enum.
2. After the individual areas, print a final line `total: T` with the sum of all areas.
Keep the existing output for circles and squares unchanged.
""", [t(["circle:2", "square:3", "rect:2x5"], "12\n9\n10\ntotal: 31\n"), t(["rect:10x1"], "10\ntotal: 10\n")], """
enum Shape {
    Circle(radius: Int)
    Square(side: Int)
    Rect(width: Int, height: Int)
}

fn number(text: Text) -> Int = value_or(parse_int(text), 0)

fn parse(text: Text) -> Shape {
    let parts = split(text, ":")
    match parts[0] {
        "circle" => .Circle(number(parts[1]))
        "square" => .Square(number(parts[1]))
        _ => {
            let size = split(parts[1], "x")
            .Rect(number(size[0]), number(size[1]))
        }
    }
}

fn area(shape: Shape) -> Int {
    match shape {
        .Circle(radius) => 3 * radius * radius
        .Square(side) => side * side
        .Rect(width, height) => width * height
    }
}

fn main() -> Unit uses io, env {
    var total = 0
    for argument in args() {
        let value = area(parse(argument))
        set total += value
        print("\\(value)")
    }
    print("total: \\(total)")
}
""", starter="""
enum Shape {
    Circle(radius: Int)
    Square(side: Int)
}

fn number(text: Text) -> Int = value_or(parse_int(text), 0)

fn parse(text: Text) -> Shape {
    let parts = split(text, ":")
    match parts[0] {
        "circle" => .Circle(number(parts[1]))
        _ => .Square(number(parts[1]))
    }
}

fn area(shape: Shape) -> Int {
    match shape {
        .Circle(radius) => 3 * radius * radius
        .Square(side) => side * side
    }
}

fn main() -> Unit uses io, env {
    for argument in args() {
        print("\\(area(parse(argument)))")
    }
}
""")

task("extend-inventory", "extend", """
The program below keeps an inventory. Each command-line argument is a command: `add:ITEM:QTY` adds QTY units of ITEM (new items are appended), `remove:ITEM:QTY` removes units. At the end it prints every item with a non-zero quantity as `ITEM QTY`, in order of first addition. Extend it:
1. Make `remove` fail when an item is unknown or has fewer units than requested: print `error: cannot remove QTY ITEM` to standard error and exit with code 1. Use a `Result` returned from the function that applies a command.
2. Add a command `clear:ITEM` that sets the quantity of a known item to 0 (unknown items are ignored).
3. Any other command prints `error: unknown command ARG` (with the whole argument) to standard error and exits with code 1.
""", [t(["add:apple:5", "add:pear:2", "remove:apple:3", "add:fig:1", "clear:pear"], "apple 2\nfig 1\n"),
      e(["add:apple:1", "remove:apple:2"], "error: cannot remove 2 apple\n"),
      e(["remove:kiwi:1"], "error: cannot remove 1 kiwi\n"),
      e(["add:apple:1", "eat:apple"], "error: unknown command eat:apple\n")], """
type Stock {
    items: List[Text]
    quantities: List[Int]
}

fn number(text: Text) -> Int = value_or(parse_int(text), 0)

fn change(stock: Stock, index: Int, quantity: Int) -> Stock = stock with { quantities: replace_at(stock.quantities, index, quantity) }

fn apply(stock: Stock, command: Text) -> Result[Stock, Text] {
    let parts = split(command, ":")
    let found = if length(parts) >= 2 { index_of(stock.items, parts[1]) } else { .None }
    if length(parts) == 3 and parts[0] == "add" {
        let quantity = number(parts[2])
        return match found {
            .Some(index) => .Ok(change(stock, index, stock.quantities[index] + quantity))
            .None => .Ok(Stock { items: append(stock.items, parts[1]), quantities: append(stock.quantities, quantity) })
        }
    }
    if length(parts) == 3 and parts[0] == "remove" {
        let quantity = number(parts[2])
        return match found {
            .Some(index) => if stock.quantities[index] < quantity { .Error("cannot remove \\(parts[2]) \\(parts[1])") } else { .Ok(change(stock, index, stock.quantities[index] - quantity)) }
            .None => .Error("cannot remove \\(parts[2]) \\(parts[1])")
        }
    }
    if length(parts) == 2 and parts[0] == "clear" {
        return match found {
            .Some(index) => .Ok(change(stock, index, 0))
            .None => .Ok(stock)
        }
    }
    .Error("unknown command \\(command)")
}

fn main() -> Unit uses io, env {
    var stock = Stock { items: [], quantities: [] }
    for argument in args() {
        match apply(stock, argument) {
            .Ok(updated) => set stock = updated
            .Error(message) => {
                exit_with(1, "error: \\(message)")
            }
        }
    }
    for index in range(0, length(stock.items)) {
        if stock.quantities[index] != 0 {
            print("\\(stock.items[index]) \\(stock.quantities[index])")
        }
    }
}
""", starter="""
type Stock {
    items: List[Text]
    quantities: List[Int]
}

fn number(text: Text) -> Int = value_or(parse_int(text), 0)

fn change(stock: Stock, index: Int, quantity: Int) -> Stock = stock with { quantities: replace_at(stock.quantities, index, quantity) }

fn apply(stock: Stock, command: Text) -> Stock {
    let parts = split(command, ":")
    let quantity = number(parts[2])
    match index_of(stock.items, parts[1]) {
        .Some(index) => if parts[0] == "add" { change(stock, index, stock.quantities[index] + quantity) } else { change(stock, index, stock.quantities[index] - quantity) }
        .None => Stock { items: append(stock.items, parts[1]), quantities: append(stock.quantities, quantity) }
    }
}

fn main() -> Unit uses io, env {
    var stock = Stock { items: [], quantities: [] }
    for argument in args() {
        set stock = apply(stock, argument)
    }
    for index in range(0, length(stock.items)) {
        if stock.quantities[index] != 0 {
            print("\\(stock.items[index]) \\(stock.quantities[index])")
        }
    }
}
""")
print("part 2 written")
