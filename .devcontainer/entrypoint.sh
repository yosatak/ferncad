#!/bin/bash
set -e

HOST_UID="${HOST_UID:-1000}"
HOST_GID="${HOST_GID:-1000}"

# Adjust dev user's UID/GID to match host
if [ "$(id -u dev)" != "$HOST_UID" ] || [ "$(id -g dev)" != "$HOST_GID" ]; then
  groupmod -g "$HOST_GID" dev 2>/dev/null || true
  usermod -u "$HOST_UID" -g "$HOST_GID" dev 2>/dev/null || true
  chown -R "$HOST_UID:$HOST_GID" /home/dev
fi

# Run command as dev user
exec gosu dev "$@"
