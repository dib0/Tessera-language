"""Writes harness/tasks/<id>/ (part 3: application tasks for draft 0.13: maps, JSON, a web handler)."""
import os, sys
sys.path.insert(0, os.path.dirname(__file__))
from make_tasks_part1 import task, t, e  # noqa: E402

task("word-frequency", "write", """
Write a program that reports the most frequent words. Every command-line argument is one word; words are compared exactly as given. Print the three most frequent words (fewer if there are fewer distinct words), one per line as `word count`, most frequent first; words with the same count are ordered alphabetically (by code point).
""", [t(["b", "a", "b", "c", "a", "b", "d"], "b 3\na 2\nc 1\n"), t(["x"], "x 1\n"), t(["k", "j", "k", "j", "i"], "j 2\nk 2\ni 1\n"), t([], "")], """
fn order_key(entry: Entry) -> Text = pad_left(to_text(1000000 - entry.count), 7, "0") + entry.word

type Entry {
word: Text
count: Int
}

fn main() -> Unit uses io, env {
var counts: Map[Text, Int] = [:]
for word in args() {
set counts = put(counts, word, value_or(get(counts, word), 0) + 1)
}
var entries: List[Entry] = []
for word in keys(counts) {
set entries += [Entry { word: word, count: value_or(get(counts, word), 0) }]
}
let sorted = sort_by(entries, order_key)
for entry in slice(sorted, 0, if length(sorted) < 3 { length(sorted) } else { 3 }) {
print("\\(entry.word) \\(entry.count)")
}
}
""")

task("json-total", "write", """
Write a program that totals an order given as JSON. The only command-line argument is a JSON array of objects with a text field `name`, a number field `price` and a whole-number field `quantity`, for example `[{"name": "pen", "price": 1.25, "quantity": 4}]`. Print `items: N` (the number of objects) and then `total: T`, where T is the sum of price times quantity with exactly two decimals (rounded half away from zero). If the argument is not valid JSON or not an array, print `error: invalid JSON` to standard error and exit with code 1; if an object lacks one of the fields or a field has the wrong type, print `error: invalid item I` (I counts from 1) to standard error and exit with code 1. Use the standard module `std/json`.
""", [t(['[{"name": "pen", "price": 1.25, "quantity": 4}, {"name": "cup", "price": 3.5, "quantity": 3}]'], "items: 2\ntotal: 15.50\n"),
      e(["[1, 2"], "error: invalid JSON\n"),
      t(["[]"], "items: 0\ntotal: 0.00\n"),
      e(['[{"name": "pen", "price": 1.25, "quantity": 4}, {"name": "cup", "price": "x", "quantity": 1}]'], "error: invalid item 2\n"),
      t(['[{"name": "a", "price": 0.333, "quantity": 3}]'], "items: 1\ntotal: 1.00\n"),
      e(['{"name": "a"}'], "error: invalid JSON\n")], """
import std/json

fn two_decimals(value: Decimal) -> Text {
let parts = split(to_text(round(value, 2)), ".")
let fraction = if length(parts) > 1 { parts[1] } else { "" }
"\\(parts[0]).\\(pad_right(fraction, 2, "0"))"
}

fn main() -> Unit uses io, env {
let arguments = args()
if length(arguments) != 1 {
exit_with(1, "error: invalid JSON")
}
let document = json.parse(arguments[0]) else exit_with(1, "error: invalid JSON")
let items = match document {
.Array(values) => values
_ => exit_with(1, "error: invalid JSON")
}
var total = 0.0
for index in range(0, length(items)) {
let item = items[index]
let problem = "error: invalid item \\(index + 1)"
let _ = json.text_field(item, "name") else exit_with(1, problem)
let price_value = json.field(item, "price") else exit_with(1, problem)
let price = json.as_decimal(price_value) else exit_with(1, problem)
let quantity = json.int_field(item, "quantity") else exit_with(1, problem)
set total += price * decimal(quantity)
}
print("items: \\(length(items))")
print("total: \\(two_decimals(total))")
}
""")

task("web-router", "web", """
Write a web application (a `handle` function, using the standard module `std/http`) with these routes. Only `GET` is allowed; any other method gets status 405 with body `method not allowed`.
- `/` gives status 200 with body `home`.
- `/hello` gives 200 with body `hello NAME`, where NAME is the `name` query parameter, or `world` without one.
- `/items/ID` gives 200 with body `item ID` when ID is a whole number, otherwise 400 with body `bad id`.
- Anything else gives 404 with body `not found`.
Bodies are plain text without a final newline. The harness runs `app request METHOD PATH` and compares the status line and the body; the examples show these two parts, separated by a blank line.
""", [t(["request", "GET", "/hello?name=Ada"], "HTTP/1.1 200 OK\n\nhello Ada"), t(["request", "GET", "/items/x7"], "HTTP/1.1 400 Bad Request\n\nbad id"),
      t(["request", "GET", "/"], "HTTP/1.1 200 OK\n\nhome"), t(["request", "GET", "/hello"], "HTTP/1.1 200 OK\n\nhello world"),
      t(["request", "GET", "/items/42"], "HTTP/1.1 200 OK\n\nitem 42"), t(["request", "GET", "/nothing/here"], "HTTP/1.1 404 Not Found\n\nnot found"),
      t(["request", "POST", "/"], "HTTP/1.1 405 Method Not Allowed\n\nmethod not allowed"), t(["request", "GET", "/items/"], "HTTP/1.1 400 Bad Request\n\nbad id")], """
import std/http

fn handle(request: http.Request) -> http.Response {
if request.method != "GET" {
return http.text(405, "method not allowed")
}
if request.path == "/" {
return http.text(200, "home")
}
if request.path == "/hello" {
return http.text(200, "hello \\(value_or(get(request.query, "name"), "world"))")
}
let parameters = http.match_path("/items/:id", request.path) else return http.text(404, "not found")
let id = parse_int(value_or(get(parameters, "id"), "")) else return http.text(400, "bad id")
http.text(200, "item \\(id)")
}
""", effects=())

print("part 3 written")
