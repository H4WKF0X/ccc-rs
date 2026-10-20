#!/bin/sh
# Run a JavaScript checker/oracle under node's permission model. Sandbox code from a level zip is
# untrusted: here it may only READ checker/ and work/. No writes, no network, no subprocesses, no
# worker threads (node --permission blocks them by default). Use this for every script that loads
# a level's sandbox code; never run such code with plain `node`.
#   sh checker/node-safe.sh checker/<script>.js [args...]
root="$(cd "$(dirname "$0")/.." && pwd)"
exec node --permission --allow-fs-read="$root/checker/" --allow-fs-read="$root/work/" "$@"
