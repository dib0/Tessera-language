Write a web application (a `handle` function, using the standard module `std/http`) with these routes. Only `GET` is allowed; any other method gets status 405 with body `method not allowed`.
- `/` gives status 200 with body `home`.
- `/hello` gives 200 with body `hello NAME`, where NAME is the `name` query parameter, or `world` without one.
- `/items/ID` gives 200 with body `item ID` when ID is a whole number, otherwise 400 with body `bad id`.
- Anything else gives 404 with body `not found`.
Bodies are plain text without a final newline. The harness runs `app request METHOD PATH` and compares the status line and the body; the examples show these two parts, separated by a blank line.
