# GaussMatrix for NixOS

GaussMatrix can be acquired by Nix from various places:

* The `flake.nix` at the root of the repo
* The `default.nix` at the root of the repo
* From GaussMatrix's binary cache

A community maintained NixOS package is available at [`gaussmatrix`](https://search.nixos.org/packages?channel=unstable&show=gaussmatrix&from=0&size=50&sort=relevance&type=packages&query=gaussmatrix)

### NixOS module

A NixOS module ships with Nixpkgs as [`services.matrix-gaussmatrix`][gaussmatrix-module],
available in 25.11 and unstable. It generates `gaussmatrix.toml` from a `settings` attrset
and runs the server under a hardened systemd unit (`DynamicUser`, `ProtectSystem=strict`,
strict `SystemCallFilter`).

Minimal configuration:

```nix
{
  services.matrix-gaussmatrix = {
    enable = true;
    settings.global = {
      server_name = "example.com";
      address = [ "127.0.0.1" "::1" ];
      port = [ 6167 ];
      allow_federation = true;
    };
  };
}
```

Notable defaults:

* User and group `gaussmatrix` (override via `services.matrix-gaussmatrix.user` / `.group`).
* Database under `/var/lib/gaussmatrix/` (override via `services.matrix-gaussmatrix.stateDirectory`).
* Listens on `127.0.0.1` and `::1` port `6167`.

Anything placed under `settings.global` is written verbatim into the `[global]` table of
`gaussmatrix.toml`, so the [configuration reference](../configuration.md) applies directly.

#### UNIX sockets

The module exposes `unix_socket_path` and `unix_socket_perms` directly:

```nix
services.matrix-gaussmatrix.settings.global = {
  unix_socket_path = "/run/gaussmatrix/gaussmatrix.sock";
  unix_socket_perms = 660;
};
```

Leave `address` unset (or `null`) when using a socket. The systemd unit already permits
`AF_UNIX`, so no further overrides are needed.

#### Migrating from `services.matrix-conduit`

`services.matrix-gaussmatrix` replaces the legacy [`services.matrix-conduit`][conduit-module]
module that older guides reference. Most settings carry over because both render the
same TOML schema. When migrating:

* Disable `services.matrix-conduit` and enable `services.matrix-gaussmatrix`.
* Confirm the database is RocksDB. GaussMatrix dropped SQLite in favor of RocksDB; if you
  ran a SQLite Conduit, migrate first with
  [conduit_toolbox](https://github.com/ShadowJonathan/conduit_toolbox/).
* Either set `services.matrix-gaussmatrix.stateDirectory` to match your existing
  `database_path`, or move the database under `/var/lib/gaussmatrix/`.


[gaussmatrix-module]: https://search.nixos.org/options?channel=unstable&query=services.matrix-gaussmatrix
[conduit-module]: https://search.nixos.org/options?channel=unstable&query=services.matrix-conduit
