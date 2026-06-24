./busybox echo "#### OS COMP TEST GROUP START basic ####"
# StarryX: BUILD.sh stages the basic ELFs + run-all.sh flat in this suite dir
# (no ./basic subdir), so run the harness in place. Invoke via `busybox sh` —
# the kernel exec does not honour the `#!` shebang for a cwd-relative
# `./run-all.sh` (busybox execvp returns "not found").
./busybox sh run-all.sh
./busybox echo "#### OS COMP TEST GROUP END basic ####"
