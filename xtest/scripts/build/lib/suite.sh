# suite.sh — shared helpers for per-suite BUILD.sh recipes.
#
# Sourced into every xtest/testsuites/<s>/BUILD.sh by build-suites.sh, after the
# uniform env (ARCH, MUSL_CC, PREFIX, SUITE_SRC, OUT_DIR) is exported. Lets each
# recipe focus on its unique build steps instead of repeating boilerplate.
#
# Conventions used by the helpers below:
#   SUITE   the suite name (set by suite_init), used to namespace log lines
#   HOST    the configure --host triple, derived from PREFIX (e.g.
#           riscv64-linux-musl); set by suite_init
#   OUT_DIR created by suite_init; staged artifacts land here
#
# All of these are POSIX sh (ash-compatible) and `set -u` clean.

# say <msg...> — namespaced progress line.
say() { echo "[$SUITE] $*"; }

# die <msg...> — namespaced error then exit 1.
die() { echo "[${SUITE:-build}] $*" >&2; exit 1; }

# suite_init <name> — validate the uniform env, derive HOST, ensure OUT_DIR.
suite_init() {
    SUITE=$1
    : "${MUSL_CC:?MUSL_CC must be set}"
    : "${PREFIX:?PREFIX must be set}"
    : "${SUITE_SRC:?SUITE_SRC must be set}"
    : "${OUT_DIR:?OUT_DIR must be set}"
    HOST=${PREFIX%-}                       # riscv64-linux-musl- -> riscv64-linux-musl
    mkdir -p "$OUT_DIR" || die "cannot mkdir $OUT_DIR"
}

# enter <dir> — cd into a required build dir (relative to SUITE_SRC or absolute).
enter() {
    case "$1" in
        /*) d=$1 ;;
        *)  d="$SUITE_SRC/$1" ;;
    esac
    [ -d "$d" ] || die "missing source dir $d"
    cd "$d" || die "cannot cd $d"
}

# need <file...> — assert each expected build output exists (run from cwd).
need() {
    for f in "$@"; do
        [ -e "$f" ] || die "missing build output: $f"
    done
}

# stage <src> <dst-name> — copy one file into OUT_DIR as <dst-name>, make it
# executable. <src> is relative to cwd (or absolute).
stage() {
    cp -a "$1" "$OUT_DIR/$2" || die "cannot stage $1"
    chmod 0755 "$OUT_DIR/$2"
}

# stage_files <file...> — copy each file into OUT_DIR under its basename, +x.
stage_files() {
    for f in "$@"; do
        [ -e "$f" ] || die "missing file to stage: $f"
        cp -a "$f" "$OUT_DIR/$(basename "$f")" || die "cannot stage $f"
        chmod 0755 "$OUT_DIR/$(basename "$f")"
    done
}

# stage_driver — copy this suite's vendored <suite>_testcode.sh into OUT_DIR.
# Every suite ships exactly one; run-suite.sh discovers it there.
stage_driver() {
    drv="$SUITE_SRC/${SUITE}_testcode.sh"
    [ -f "$drv" ] || die "missing driver ${SUITE}_testcode.sh"
    stage "$drv" "${SUITE}_testcode.sh"
}

# retry <n> -- <cmd...> — run cmd, retrying up to n times. The emulated cross-gcc
# intermittently dies with an internal-compiler-error SIGSEGV on non-amd64 hosts;
# `make` resumes from already-built objects, so a retry converges. A real error
# fails all n attempts identically.
retry() {
    n=$1; shift
    [ "$1" = "--" ] && shift
    i=0
    while [ "$i" -lt "$n" ]; do
        if "$@"; then return 0; fi
        i=$((i + 1))
        [ "$i" -lt "$n" ] && say "attempt $i failed (likely transient); retrying"
    done
    die "failed after $n attempts: $*"
}
