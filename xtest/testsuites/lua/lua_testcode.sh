./busybox echo "#### OS COMP TEST GROUP START lua ####"

# StarryX: invoke the nested test.sh via `busybox sh` rather than `./test.sh`.
# The kernel's exec does not honour the `#!/bin/sh` shebang for a cwd-relative
# `./script.sh` (busybox's execvp returns "not found"); an explicit interpreter
# sidesteps it. test.sh prints `testcase lua <script> success|fail` (adapt_lua).
for t in date file_io max_min random remove round_num sin30 sort strings; do
    ./busybox sh test.sh "$t.lua"
done

./busybox echo "#### OS COMP TEST GROUP END lua ####"
