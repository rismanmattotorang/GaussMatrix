# Reverse Proxy Setup - Traefik

[<= Back to Generic Deployment Guide](generic.md#setting-up-the-reverse-proxy)

## Installation

Install Traefik via your preferred method. You can read the official [docker quickstart guide](https://doc.traefik.io/traefik/getting-started/docker/) or the [in-depth walkthrough](https://doc.traefik.io/traefik/setup/docker/)

## Configuration
### TLS certificates

You can setup auto renewing certificates with different kinds of [acme challenges](https://doc.traefik.io/traefik/reference/install-configuration/tls/certificate-resolvers/acme/).
### Router configurations
Add gaussmatrix to your traefik's network.

```yaml
services:
    gaussmatrix:
    # ...
    networks:
        - proxy # your traefik network name
networks:
    proxy: # your traefik network name
        external: true
```

Be sure to change the `your.server.name` to your actual gaussmatrix domain. and the `yourcertresolver` should be changed to whatever you named it in your traefik config.

You only have to do any one of these methods below.


### Labels
To use labels with traefik you need to configure a [docker provider](https://doc.traefik.io/traefik/reference/install-configuration/providers/docker/).

Then add the labels in your gaussmatrix's docker compose file.
```yaml
services:
    gaussmatrix:
        # ...
        labels:
            - "traefik.enable=true"
            - "traefik.http.routers.gaussmatrix.entrypoints=web"
            - "traefik.http.routers.gaussmatrix.rule=Host(`your.server.name`)"
            - "traefik.http.routers.gaussmatrix.middlewares=https-redirect@file"
            - "traefik.http.routers.gaussmatrix-secure.entrypoints=websecure"
            - "traefik.http.routers.gaussmatrix-secure.rule=Host(`your.server.name`)"
            - "traefik.http.routers.gaussmatrix-secure.tls=true"
            - "traefik.http.routers.gaussmatrix-secure.service=gaussmatrix"
            - "traefik.http.services.gaussmatrix.loadbalancer.server.port=6167"
            - "traefik.http.routers.gaussmatrix-secure.tls.certresolver=yourcertresolver"
            - "traefik.docker.network=proxy"
```
### Config File
To use the config file you need to configure a [file provider](https://doc.traefik.io/traefik/reference/install-configuration/providers/others/file/).

Then add this into your config file.
```yaml
http:
    routers:
        gaussmatrix:
            entryPoints:
                - "web"
                - "websecure"
            rule: "Host(`your.server.name`)"
            middlewares:
                - https-redirect
            tls:
                certResolver: "yourcertresolver"
            service: gaussmatrix
    services:
        gaussmatrix:
            loadBalancer:
                servers:
            # this url should point to your gaussmatrix installation.
            # this should work if your gaussmatrix container is named gaussmatrix and is in the same network as traefik.
                    - url: "http://gaussmatrix:6167"
                passHostHeader: true
```

### Client IP source

If Traefik is the only way clients can reach GaussMatrix, set
`ip_source = "rightmost_x_forwarded_for"` in `gaussmatrix.toml` so GaussMatrix uses the
trusted `X-Forwarded-For` value.

### Federation

If you will use a .well-known file you can use traefik to redirect .well-known/matrix to gaussmatrix built-in .well-known file.

replace the rule in either of the methods from
```
Host(`your.server.name`)
```
to
```
Host(`your.gaussmatrix.domain`) || Host(`your.server.name`) && PathPrefix(`/.well-known/matrix`)
```
If you are not using a .well-known file you will need to add and expose port 8448 to a [traefik entrypoint](https://doc.traefik.io/traefik/reference/install-configuration/entrypoints/).

You can then add these to your preferred traefik config method.
you should replace `matrixfederationentry` with what you named your entrypoint.

Labels:
```yaml
            - "traefik.http.routers.matrix-federation.entrypoints=matrixfederationentry"
            - "traefik.http.routers.matrix-federation.rule=Host(`your.server.name`)"
            - "traefik.http.routers.matrix-federation.tls=true"
            - "traefik.http.routers.matrix-federation.service=matrix-federation"
            - "traefik.http.services.matrix-federation.loadbalancer.server.port=6167"
            - "traefik.http.routers.matrix-federation.tls.certresolver=yourcertresolver"
```
Config file:
```yaml
        entryPoints:
            - "web"
            - "websecure"
            - "matrixfederationentry"
```
> [!IMPORTANT]
>
> [Encoded Character Filtering](https://doc.traefik.io/traefik/security/request-path/#encoded-character-filtering)
> options must be set to `true`. This only applies to traefik version 3.6.4 to 3.6.6 and 2.11.32 to 2.11.34


## Verification

After starting Traefik, verify it's working by checking:

```bash
curl https://your.server.name/_gaussmatrix/server_version
curl https://your.server.name:8448/_gaussmatrix/server_version
```

---

[=> Continue with "You're Done"](generic.md#you-are-done)
