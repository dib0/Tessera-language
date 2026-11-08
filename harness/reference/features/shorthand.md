
## Shorthand variants

Where the expected type is already known, write a variant without its type: `.Red` instead of `Color.Red`, `.Some(x)` instead of `Option.Some(x)`, `.Ok(v)` / `.Error(e)` for `Result`. Prefer the shorthand wherever it is allowed.

```tessera
fn name_of(color: Color) -> Text {
match color {
.Red => "red"
.Custom(name) => name
}
}

fn parse(text: Text) -> Option[Color] = if text == "red" { .Some(.Red) } else { .None }

let favourite: Color = .Red
let pixel = Pixel { color: .Red, bright: true }
if favourite == .Red {
print("red")
}
```

The type is known in patterns, function arguments, return values, annotated bindings, record fields, list elements of a known list type, and on the right side of `==` / `!=`. Without a known type (for example `let c = .Red` or `.Red == c`) write the full form; the compiler reports E0518 otherwise.
