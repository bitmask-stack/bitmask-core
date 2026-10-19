# Builder
FROM rust:1.99-slim-bookworm AS builder
ARG BUILDER_DIR=/srv/bitmask
ARG BUILDER_SRC=/opt/src/bitmask

RUN apt-get update -y && \
  apt-get install -y pkg-config make g++ libssl-dev

WORKDIR $BUILDER_DIR
WORKDIR $BUILDER_SRC
COPY . .

RUN cargo install --locked --features server --path . --root ${BUILDER_DIR}

# Runtime
FROM rust:1.99-slim-bookworm AS runtime

ARG BUILDER_DIR=/srv/bitmask
ARG BIN_DIR=/usr/local/bin
ARG DATA_DIR=/tmp/bitmaskd/carbonado/
ARG USER=bitmask

RUN apt-get update -y && apt-get install -y iputils-ping telnet \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

RUN adduser --home "${DATA_DIR}" --shell /bin/bash --disabled-login \
            --gecos "${USER} user" ${USER}

COPY --from=builder --chown=${USER}:${USER} \
     "${BUILDER_DIR}/bin/" "${BIN_DIR}"

USER ${USER}
VOLUME ${DATA_DIR}
EXPOSE 7070

WORKDIR ${BIN_DIR}

ENTRYPOINT ["bitmaskd"]
