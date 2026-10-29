#!/bin/sh
# EasyTierUI deployment: firewalld rules for vpn.example.com.
# Rendered, not applied. Read it, then run it as root. It opens only:
#   80/tcp   HTTP: certificate issuance and the redirect to HTTPS
#   443/tcp  HTTPS: the dashboard (/api/ is closed at the proxy until hardening is done)
#   22020/udp  config server (remote nodes enroll here), after hardening
# It touches only the default zone's permanent rules and does not change SSH.
# The REST API (11211) and the node's RPC portal (15888) stay closed.
# Docker publishes ports through its own iptables rules, which ufw and firewalld do not filter:
# the compose file therefore publishes the config server on 127.0.0.1 until hardening is done.
set -eu

firewall-cmd --permanent --add-port=80/tcp
firewall-cmd --permanent --add-port=443/tcp
# HARDENING SWITCH: the config server stays closed until hardening is done (README.md); then re-render
# with hardening done or uncomment the next line. It binds every interface, so this rule is the gate.
# firewall-cmd --permanent --add-port=22020/udp
firewall-cmd --reload
