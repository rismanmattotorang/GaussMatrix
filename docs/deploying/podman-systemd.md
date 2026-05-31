# Podman, Quadlets, and systemd

For a rootless setup, we can use quadlets and systemd to manage the container lifecycle.

> [!IMPORTANT]
> If this is the first container managed with quadlets for your user, ensure that linger
> is enabled so your containers are not killed after logging out.
>
> `sudo loginctl enable-linger <username>`  

### Step One

Copy quadlet files to `~/.config/containers/systemd/gaussmatrix`

**gaussmatrix.container**

<details>
<summary>gaussmatrix container quadlet</summary>

```
{{#include ../../quadlet/gaussmatrix.container}}
```

</details>

**gaussmatrix-db.volume**

<details>
<summary>gaussmatrix database volume quadlet</summary>

```
{{#include ../../quadlet/gaussmatrix-db.volume}}
```

</details>

**gaussmatrix.env**

<details>
<summary>gaussmatrix environment variable quadlet</summary>

```env
{{#include ../../quadlet/gaussmatrix.env}}
```

</details>


```
mkdir -p ~/.config/containers/systemd/gaussmatrix
```

### Step Two

Modify `gaussmatrix.env` and [`gaussmatrix.toml`](generic.md#creating-the-gaussmatrix-configuration-file)
to desired values. This can be saved in your user home directory if desired.

### Step Three

- Reload daemon to generate our systemd unit files:

```
systemctl --user daemon-reload
```

### Step Four

- Start gaussmatrix:

```
systemctl --user start gaussmatrix
```

## Logging 

To check the logs, run:
```
systemctl --user status gaussmatrix
```
or

```
podman logs gaussmatrix-homeserver
```

## Troubleshooting systemd unit file generation

Look for errors in the output:
`/usr/lib/systemd/system-generators/podman-system-generator --user --dryrun`

