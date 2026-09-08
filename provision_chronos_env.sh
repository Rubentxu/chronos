#!/usr/bin/env bash
#
# provision_chronos_env.sh
#
# Review + provision the host capabilities the Chronos SDDK/M4 work needs.
#
# System audited: Bazzite (Fedora Atomic / ostree), SELinux Enforcing, user in wheel.
#
# Strategy:
#   * User-space toolchain (clang/llvm, gdb, strace, ...)  -> Homebrew (NO root).
#   * System/root-only capabilities (eBPF/perf attach, pwless sudo, yama) ->
#     offered as OPTIONAL steps that prompt for sudo (you type your password once).
#
# Usage:
#   bash provision_chronos_env.sh            # non-interactive default: audit + brew user tools
#   bash provision_chronos_env.sh --admin    # also attempt the sudo system steps (prompts)
#   bash provision_chronos_env.sh --report   # only print the capability report, change nothing
#
set -uo pipefail

say()  { printf '\033[1;34m[provision]\033[0m %s\n' "$*"; }
ok()   { printf '\033[1;32m  ok\033[0m     %s\n' "$*"; }
warn() { printf '\033[1;33m  warn\033[0m   %s\n' "$*"; }
need() { printf '\033[1;31m  NEED\033[0m   %s\n' "$*"; }

ADMIN=0
REPORT=0
for a in "$@"; do
  case "$a" in
    --admin) ADMIN=1 ;;
    --report) REPORT=1 ;;
  esac
done

# ---------------------------------------------------------------------------
# 1. OS / toolchain audit
# ---------------------------------------------------------------------------
audit() {
  echo
  say "Capability report"
  printf '  os         : %s\n' "$(grep PRETTY_NAME /etc/os-release 2>/dev/null | cut -d= -f2 | tr -d '"')"
  [ -e /run/ostree-booted ] && printf '  ostree     : yes (atomic host; /usr read-only)\n'
  printf '  selinux    : %s\n' "$(getenforce 2>/dev/null)"
  printf '  yama ptrace: %s (0 = classic, child+same-uid ok)\n' "$(cat /proc/sys/kernel/yama/ptrace_scope 2>/dev/null || echo n/a)"

  local t
  for t in gcc cc go rustc cargo objdump readelf addr2line strace gdb clang llvm-config; do
    local p; p=$(command -v "$t" 2>/dev/null)
    if [ -n "$p" ]; then ok "$t -> $p"; else need "$t (missing)"; fi
  done
  printf '  rustup     : %s\n' "$(rustup toolchain list 2>/dev/null | tr '\n' ' ')"
}

# ---------------------------------------------------------------------------
# 2. Homebrew user tooling (NO root)
# ---------------------------------------------------------------------------
brew_present() { command -v brew >/dev/null 2>&1; }

brew_ensure() { # $1 = formula
  if command -v "$1" >/dev/null 2>&1 || brew list --formula "$1" >/dev/null 2>&1; then
    ok "$1 already installed"
  else
    say "brew install $1 (no root)..."
    brew install "$1" || warn "brew install $1 failed (network?)"
  fi
}

provision_brew() {
  echo
  say "Homebrew user tooling (no root)"
  if ! brew_present; then
    need "Homebrew is not on PATH. Install it (no root): https://brew.sh then re-run."
    return 0
  fi
  brew_ensure llvm        # provides clang/clang++/llvm-config (M4B tooling)
  brew_ensure gdb
  brew_ensure strace
  # Re-check PATH: brew bin may not be exported in this shell.
  export PATH="$(brew --prefix 2>/dev/null)/bin:$PATH"
  command -v clang >/dev/null 2>&1 && ok "clang -> $(command -v clang)"
}

# ---------------------------------------------------------------------------
# 3. Optional ADMIN system steps (prompt for sudo password once)
# ---------------------------------------------------------------------------
admin_steps() {
  echo
  say "Admin (sudo) optional steps — you will be prompted for your password."
  [ "$ADMIN" = 1 ] || { say "skip: pass --admin to run these."; return 0; }

  # 3a. yama ptrace: ensure classic (already 0 on this host)
  local scope; scope=$(cat /proc/sys/kernel/yama/ptrace_scope 2>/dev/null)
  if [ "$scope" != "0" ]; then
    say "Setting kernel.yama.ptrace_scope=0 (child + same-uid ptrace)..."
    sudo sysctl -w kernel.yama.ptrace_scope=0 2>/dev/null \
      && echo 'kernel.yama.ptrace_scope=0' | sudo tee /etc/sysctl.d/99-chronos.conf >/dev/null \
      || warn "could not set ptrace_scope (SELinux/privilege)"
  else
    ok "ptrace_scope already 0"
  fi

  # 3b. Optional passwordless sudo (convenience only; NOT required for brew path)
  read -r -p "Enable passwordless sudo for $(id -un)? [y/N] " ans
  if [ "${ans,,}" = "y" ]; then
    echo "$(id -un) ALL=(ALL) NOPASSWD:ALL" | sudo tee "/etc/sudoers.d/90-chronos-$(id -un)" >/dev/null \
      && sudo chmod 440 "/etc/sudoers.d/90-chronos-$(id -un)" \
      && ok "passwordless sudo enabled" || warn "could not write sudoers.d"
  fi

  # 3c. eBPF / perf at runtime
  echo
  warn "eBPF / perf / PTRACE_ATTACH to foreign pids need ROOT at RUNTIME."
  warn "SELinux is Enforcing. Run eBPF-capable probes under an admin shell:"
  warn "    sudo -E env PATH=\"$PATH\" <probe command>"
  warn "and ensure the kernel allows it (no unprivileged_bpf_disabled blocker)."
}

# ---------------------------------------------------------------------------
# 4. Runtime proof: child ptrace works in this environment
# ---------------------------------------------------------------------------
child_ptrace_probe() {
  echo
  say "Probe: ptrace of a child process (PTRACE_LAUNCH/TRACEME path)"
  local d; d="$(mktemp -d)"
  cat > "$d/p.c" <<'EOF'
#define _GNU_SOURCE
#include <sys/ptrace.h>
#include <sys/wait.h>
#include <unistd.h>
#include <stdio.h>
int main(void){ pid_t p=fork();
 if(p==0){ ptrace(PTRACE_TRACEME,0,0,0); raise(SIGSTOP); _exit(0);}
 int st=0; waitpid(p,&st,0);
 if(WIFSTOPPED(st)){ printf("PTRACE child OK (stopped sig=%d)\n", WSTOPSIG(st));
   ptrace(PTRACE_CONT,p,0,0); waitpid(p,&st,0); return WIFEXITED(st)?0:1;}
 printf("NOT stopped (status=%x)\n", st); return 1; }
EOF
  if gcc -o "$d/p" "$d/p.c" 2>/dev/null && "$d/p" >/dev/null 2>&1; then
    ok "child ptrace works — native child capture (chronos-native) is runnable here"
  else
    warn "child ptrace probe failed — a seccomp/security layer may block ptrace even for children"
  fi
  rm -rf "$d"
}

# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------
audit
if [ "$REPORT" = 1 ]; then echo; echo "Report only. Re-run without --report to provision."; exit 0; fi
provision_brew
child_ptrace_probe
admin_steps

echo
say "Next for the real Chronos capture milestone (no root):"
ok "compile a -g C fixture with gcc and drive chronos-native PTRACE_LAUNCH/TRACEME to capture real FunctionEntry events"
say "Root-only later (when you want eBPF/perf/foreign attach): run under 'sudo -E'."
