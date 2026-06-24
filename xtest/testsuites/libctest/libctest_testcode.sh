
./busybox echo "#### OS COMP TEST GROUP START libctest ####"
# StarryX: run the generated runner scripts via `busybox sh` (the kernel exec
# does not honour the `#!` shebang for a cwd-relative `./script.sh`).
./busybox sh run-static.sh
./busybox sh run-dynamic.sh
./busybox echo "#### OS COMP TEST GROUP END libctest ####"

