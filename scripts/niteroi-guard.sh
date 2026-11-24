#!/bin/sh
# Maintain only the dedicated Niteroi table; preserve all other rules.
set -eu
uid=$(id -u niteroi)
[ "$uid" -gt 0 ]
{
    if /usr/sbin/nft list table inet niteroi_guard >/dev/null 2>&1; then
        printf '%s\n' 'delete table inet niteroi_guard'
    fi
    printf '%s\n' 'add table inet niteroi_guard'
    printf '%s\n' 'add chain inet niteroi_guard output { type filter hook output priority 0; policy accept; }'
    printf 'add rule inet niteroi_guard output meta skuid %s meta l4proto tcp ct state != established counter reject\n' "$uid"
} | /usr/sbin/nft -f -
