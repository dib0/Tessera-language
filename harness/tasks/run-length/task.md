Write a run-length encoder and decoder. The program is called with two arguments: a mode (`encode` or `decode`) and a text.
- `encode` replaces every run of the same character by the run length followed by the character: `aaabcc` becomes `3a1b2c`. The input contains no digits.
- `decode` does the reverse: `12x2y` becomes `xxxxxxxxxxxxyy`. Counts can have several digits.
Print the result on one line. An empty text prints an empty line.
