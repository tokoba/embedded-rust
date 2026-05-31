# xtask embedded target self-exclude fix

`cargo clippy --workspace --target thumbv7em-none-eabihf --bins` includes the `xtask` binary itself because `xtask` is a workspace member.
That attempts to compile the std-based host tool for a bare-metal target and causes many errors.

Apply `xtask-exclude-self-fix.patch`, or manually add `--exclude xtask` to both embedded commands in `xtask/src/main.rs`.
