
## Text interpolation

Inside a text literal, `\(expression)` inserts the value of an `Int`, `Bool` or `Text` expression. Prefer it over joining text with `+` and `to_text`.

```tessera
let message = "found \(count) items in \(name), done: \(count == total)"
print("total: \(a + b)")
```

The expression may contain calls and its own text literals: `"\(join(parts, ", "))"`. Braces `{}` in text have no special meaning. Lists and other values must be converted to `Text` first.
