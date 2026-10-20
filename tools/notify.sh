#!/bin/sh
# Claude Code hook: make a sound when Claude needs you (permission prompt) or has finished its turn.
# Never fails: a missing player just means silence.
for sound in /usr/share/sounds/freedesktop/stereo/complete.oga /usr/share/sounds/freedesktop/stereo/bell.oga; do
  [ -f "$sound" ] || continue
  for player in pw-play paplay; do
    if command -v "$player" >/dev/null 2>&1; then "$player" "$sound" >/dev/null 2>&1 & exit 0; fi
  done
done
if command -v afplay >/dev/null 2>&1; then afplay /System/Library/Sounds/Glass.aiff >/dev/null 2>&1 & exit 0; fi
printf '\a' > /dev/tty 2>/dev/null
exit 0
