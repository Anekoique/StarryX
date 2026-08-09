# id: starry-test
#
# Kernel-embedded boot script for `make tests` / `make run-tests`.
# Selected at build time by the `init-test` cargo feature
# (see scripts/make/cargo.mk and Cargo.toml `[features]`).

export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export LD_LIBRARY_PATH=/lib:/usr/lib
export HOME=/root

cd /root/tests || {
    echo "[test.sh] /root/tests not present; falling through to interactive shell"
    exec sh
}

if [ -f ./scripts/run-all.sh ]; then
    # Invoke via explicit `sh` — some StarryX kernels resolve shebangs
    # differently than full Linux; sourcing through busybox's sh is robust.
    sh ./scripts/run-all.sh
else
    echo "[test.sh] ./scripts/run-all.sh not found; nothing to run"
fi

exec sh
