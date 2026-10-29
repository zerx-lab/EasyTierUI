# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

## Users

Both audiences are first-class (confirmed), and every platform serves both (confirmed):

- **Individuals and small teams** connecting their own devices — remote access to home or office machines, a homelab,
  LAN games, a few laptops and phones. They want to create or join a network in about a minute and see at a glance
  whether their peers are reachable and how (direct or relayed). This is the default path.
- **Administrators and operators** managing many nodes and several networks through easytier-web, and hosting or
  deploying that control plane themselves. They need lists, per-node status, config editing, credentials, diagnosis
  and a safe deployment. This depth sits one level in, never in the first-run path.

## Product Purpose

EasyTierUI is a client for EasyTier (<https://github.com/EasyTier/EasyTier>), a decentralized mesh VPN, on macOS,
Windows, Linux, iOS, Android and the web. It exists so EasyTier can be deployed, configured and used quickly,
correctly and safely without the command line — both the data plane (`easytier-core`, devices in the mesh) and the
control plane (`easytier-web`, which pushes configs to enrolled devices).

Every platform has the same UI and every feature (confirmed), organized as roles the user switches between and
combines:

- **Node**: this device in the mesh, with local profiles or managed by a control plane.
- **Console**: signed in to one or more easytier-web servers, managing their machines, networks and credentials.
- **Control-plane host**: running easytier-web — on a desktop from inside the app, or on a Linux server through
  deployment artifacts the app generates, where EasyTierUI's web build is the dashboard.

Where a platform physically cannot do something locally (a browser cannot join a mesh, a phone cannot host a server),
the same page acts on a remote target and says so.

Most installations are one end of a network, so the first run asks one question (confirmed): **client** (the node
only, preselected) or **control** (every role). Settings switches between them or picks roles one by one
(**custom**). The mode only decides what the shell lists: hiding a role stops nothing, and a hidden role that still
runs something here stays one row away. The web build is a control plane's dashboard and starts in control mode.

Success means: a first-time user is connected with safe defaults in about a minute; a control plane can be stood up
without leaving default accounts or open APIs behind; no setting silently exposes the user's machine or network
(management ports, default accounts, exit node, subnet proxy, listeners, weak secrets); what the app shows is what
EasyTier reports; and the user can upgrade EasyTier without waiting for an app release.

## Positioning

- **The UI and EasyTier are decoupled; EasyTier is upgraded freely** (confirmed). On the desktops the app runs and
  supervises the official `easytier-core` and `easytier-web` releases as separate processes, verifies and installs
  versions side by side, and reads state through the matching `easytier-cli` and easytier-web's REST API. Mobile
  embeds EasyTier's library only because iOS cannot spawn processes and Android passes its VPN interface in process.
- One client for both planes on every platform; the official GUI covers only the node (local or managed).
- One native app on every platform (Native Shell), not a web view.
- Safe by default, explicit when not: every setting that widens exposure says what it does where it is turned on.

## Operating Context

- Desktop: creating a TUN interface needs root / Administrator / `CAP_NET_ADMIN` (Windows also `wintun.dll`); only the
  core is elevated, through the OS's mechanism, with consent. `no_tun` mode works unprivileged.
- Android: VpnService supplies the TUN interface. iOS: upstream's library has no packet tunnel (loopback port
  forwarding only).
- Web: no TUN and no sockets in the browser. The web build is served on a Linux server next to easytier-web, as its
  dashboard; the server's own core (when it is a node or relay) is managed through that easytier-web.
- Servers: headless Linux; easytier-web from the official `easytier/easytier` image (pinned by digest) or a verified
  release binary under systemd, behind an HTTPS reverse proxy.
- Networks often sit behind NAT; peers meet through a shared node the user chooses or a self-hosted one, and traffic is
  direct (P2P) or relayed.
- EasyTier facts the product depends on (2.6.4, May 2026; checked against the release binaries): TOML config files,
  `--check-config`, an unauthenticated RPC portal (15888 by default), `easytier-cli -o json`; easytier-web seeds
  `admin` / `admin` and `user` / `user`, binds its REST API to `0.0.0.0` by default and its config server on every
  interface, and its login takes an MD5 of the password; release zips carry sha256 digests on GitHub; LGPL-3.0.

## Capabilities and Constraints

- Node: profiles (create, join, import from TOML / link / QR, export, edit, delete), one running instance per profile;
  managed instances (enroll with a config-server URL, leave management); live node info, virtual IP, peers with route
  type, latency, loss and traffic; logs (redacted). On the desktops a TUN network runs as an OS service installed with
  the user's consent (decided); on phones the in-app node runs without TUN until a VPN service host exists.
- Console: several easytier-web servers; machines, networks, configs, credentials (per network, through the machine's
  RPC proxy). easytier-web 2.6.4 has no device-blocking endpoint: disabling or deleting a machine's network is the
  control it offers.
- Control-plane host: start / stop / upgrade easytier-web on a desktop with first-run hardening (replace default
  passwords, loopback API, registration off); generate server deployment artifacts (Docker Compose, systemd, reverse
  proxy) with the same defaults.
- EasyTier management on the desktops: detect, download and verify, install side by side, switch, roll back.
- Languages: zh-CN and en (agent default during setup; confirm).
- Decided: mobile embeds EasyTier's library through a UniFFI bridge crate; desktop TUN runs as an OS service.
- Open decisions: an iOS packet tunnel and an Android VpnService host (NSH 0.3 packaging cannot declare either);
  tray / menu-bar presence and autostart (services are installed with autostart off); whether the app applies
  artifacts to a server itself (SSH) or only generates them; distribution channels and signing; the app's relationship
  to the EasyTier project; the LGPL-3.0 review of shipping EasyTier's library statically linked on mobile.

## Brand Commitments

- Name: EasyTierUI. Until the relationship is decided, it does not claim to be an official EasyTier app and does not use
  EasyTier's logo.

## Evidence on Hand

None yet: no users, testimonials, screenshots, benchmarks or public shared-node list. Do not invent them; label
illustrative values as examples.

## Product Principles

1. **Safe by default, explicit when not.** Exposure is opt-in, explained and reversible.
2. **EasyTier is the truth.** The UI shows what the core and the control plane report, with its age, and never pretends.
3. **Replaceable EasyTier.** Nothing in the product assumes one EasyTier version.
4. **A minute to connected.** The first-run path has one decision per screen; advanced settings wait.
5. **Same features, native everywhere.** Every role on every platform, with each platform's navigation, controls and
   accessibility; a feature the platform cannot run locally acts on a remote target instead of disappearing.

## Accessibility & Inclusion

- Status is never color alone; every action works with keyboard, touch and screen readers.
- Text follows the user's text size; touch targets are at least 44 × 44 pt; reduced motion is honored.
- Chinese and English are both first-class, including layout room for either.
