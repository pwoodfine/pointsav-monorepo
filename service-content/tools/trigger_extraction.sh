#!/usr/bin/env bash
# Purpose: Orchestrate Rust engine and anchor to Data Mesh.
FILENAME=$1
PROTOCOL=$2
SILO_PATH=$3

ENGINE_DIR="/home/foundry/node-gcp-free/factory-pointsav/pointsav-monorepo/service-content"
DNA_PATH="/home/foundry/node-foundry/node-gcp-free/factory-pointsav/pointsav-design-system/tokens/linguistic"
PROTO_FILE="${DNA_PATH}/protocol-${PROTOCOL,,}.yaml"

echo "⚙️  SURGICAL ENGINE: Processing $FILENAME..."
cd "$ENGINE_DIR"
if [ -f "./target/release/service-content" ]; then
    ./target/release/service-content "$PROTO_FILE" "./input/$FILENAME" "./outbox"
else
    # Fallback to cargo with specific package flag to ignore workspace bloat
    cargo run --release -p service-content -- "$PROTO_FILE" "./input/$FILENAME" "./outbox"
fi

LATEST=$(ls -t ./outbox | head -n 1)
if [ -n "$LATEST" ]; then
    mkdir -p "$SILO_PATH"
    cp "./outbox/$LATEST" "$SILO_PATH/"
    echo "⚓ ANCHORED: $LATEST moved to $SILO_PATH"
fi
