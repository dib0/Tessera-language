Write a program that processes bank transactions given as command-line arguments, in order. The balance starts at 0. Each argument is `deposit:AMOUNT` or `withdraw:AMOUNT`, where AMOUNT is a positive decimal integer.
- After all transactions, print `balance: B`.
- If a withdrawal is larger than the current balance, print `error: insufficient funds at transaction N` (N counts from 1) and exit with code 1 immediately.
- If an argument is malformed (unknown kind, missing or non-numeric amount, amount 0), print `error: invalid transaction at N` and exit with code 1 immediately.
