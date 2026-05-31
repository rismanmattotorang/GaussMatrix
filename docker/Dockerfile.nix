# syntax = docker/dockerfile:1.11-labs

FROM input AS nix-base

RUN \
--mount=type=cache,dst=/nix,sharing=shared \
--mount=type=cache,dst=/root/.cache/nix,sharing=shared \
--mount=type=cache,dst=/root/.local/state/nix,sharing=shared \
<<EOF
	set -eux
	curl --proto '=https' --tlsv1.2 -L https://nixos.org/nix/install > nix-install
	sh ./nix-install --daemon
	rm nix-install
EOF


FROM nix-base AS build-nix

WORKDIR /usr/src/gaussmatrix
COPY --link --from=source /usr/src/gaussmatrix .
RUN \
--mount=type=cache,dst=/nix,sharing=shared \
--mount=type=cache,dst=/root/.cache/nix,sharing=shared \
--mount=type=cache,dst=/root/.local/state/nix,sharing=shared \
<<EOF
	set -eux

	nix-build \
		--verbose \
		--cores 0 \
		--max-jobs $(nproc) \
		--log-format raw \
		.

	cp -afRL --copy-contents result /opt/gaussmatrix
EOF


FROM nix-base AS smoke-nix

WORKDIR /usr/src/gaussmatrix
COPY --link --from=source /usr/src/gaussmatrix .
ENV GAUSSMATRIX_DATABASE_PATH="/tmp/gaussmatrix/smoketest.db"
ENV GAUSSMATRIX_LOG="info"
RUN \
--mount=type=cache,dst=/nix,sharing=shared \
--mount=type=cache,dst=/root/.cache/nix,sharing=shared \
--mount=type=cache,dst=/root/.local/state/nix,sharing=shared \
<<EOF
    set -eux
    alias nix="nix --extra-experimental-features nix-command --extra-experimental-features flakes"

    nix run \
        --verbose \
        --cores 0 \
        --max-jobs $(nproc) \
        --log-format raw \
        .#all-features \
            -- \
            -Otest='["smoke", "fresh"]' \
            -Oserver_name=\"localhost\" \
            -Oerror_on_unknown_config_opts=true \
EOF


FROM nix-base AS nix-pkg

WORKDIR /usr/src/gaussmatrix
COPY --link --from=source /usr/src/gaussmatrix .
RUN \
--mount=type=cache,dst=/nix,sharing=shared \
--mount=type=cache,dst=/root/.cache/nix,sharing=shared \
--mount=type=cache,dst=/root/.local/state/nix,sharing=shared \
<<EOF
    set -eux
    alias nix="nix --extra-experimental-features nix-command --extra-experimental-features flakes"

    ID=$(nix-store --realise $(nix path-info --derivation))

    mkdir -p gaussmatrix
    nix-store --export $ID > gaussmatrix/gaussmatrix.drv
    tar -cvf /opt/gaussmatrix.nix.tar gaussmatrix
EOF
