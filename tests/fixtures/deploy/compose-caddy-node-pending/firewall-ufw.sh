#!/bin/sh
# EasyTierUI deployment: ufw rules for vpn.example.com.
# Rendered, not applied. Read it, then run it as root. It opens only:
#   80/tcp   HTTP: certificate issuance and the redirect to HTTPS
#   443/tcp  HTTPS: the dashboard (/api/ is closed at the proxy until hardening is done)
#   22020/udp  config server (remote nodes enroll here), after hardening
# It does not enable ufw and does not touch SSH: allow your SSH port yourself BEFORE `ufw enable`.
# The REST API (11211) and the node's RPC portal (15888) stay closed.
# Docker publishes ports through its own iptables rules, which ufw and firewalld do not filter:
# the compose file therefore publishes the config server on 127.0.0.1 until hardening is done.
set -eu

ufw allow 80/tcp comment 'EasyTier proxy http'
ufw allow 443/tcp comment 'EasyTier proxy https'
# HARDENING SWITCH: the config server stays closed until hardening is done (README.md); then re-render
# with hardening done or uncomment the next line. It binds every interface, so this rule is the gate.
# ufw allow 22020/udp comment 'EasyTier config server'
