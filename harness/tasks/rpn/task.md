Write a calculator for expressions in reverse Polish notation. Each command-line argument is one token: a non-negative decimal integer, or one of the operators `+`, `-`, `*`, `/` (integer division, truncating toward zero). Evaluate the expression and print the single resulting value.
Errors print one line and exit with code 1:
- `error: stack underflow` when an operator needs more values than available,
- `error: division by zero`,
- `error: invalid token TOKEN` for anything else,
- `error: expected one result` when the expression leaves zero or several values.
