#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

# install.sh — PointSav Network OS installer
# curl -fsSL https://software.pointsav.com/releases/os-network-admin/install.sh | bash
set -euo pipefail

PRODUCT="os-network-admin"
VERSION="latest"
PLATFORM="x86_64"
BINARY_NAME="os-network-admin"
BASE_URL="https://software.pointsav.com/releases"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

OS="$(uname -s)"
ARCH="$(uname -m)"

if [[ "$OS" != "Linux" || "$ARCH" != "x86_64" ]]; then
  echo "error: this installer only supports Linux x86_64 (detected: ${OS} ${ARCH})" >&2
  exit 1
fi

REQUEST_URL="${BASE_URL}/${PRODUCT}/${VERSION}/${PLATFORM}"

# The download route auto-resolves "latest" to a concrete version via redirect, but the
# MANIFEST route does not — resolve it here so SHA verification below targets the right
# version directory instead of silently no-op'ing.
RESOLVED_VERSION="$VERSION"
if [[ "$VERSION" == "latest" ]]; then
  REDIRECT_LOCATION="$(curl -s -o /dev/null -w '%{redirect_url}' "$REQUEST_URL" || true)"
  if [[ -n "$REDIRECT_LOCATION" ]]; then
    RESOLVED_VERSION="$(printf '%s' "$REDIRECT_LOCATION" | awk -F'/' '{print $(NF-1)}')"
  fi
fi

TMP_FILE="$(mktemp)"
trap 'rm -f "$TMP_FILE"' EXIT

echo "Downloading ${PRODUCT} (${RESOLVED_VERSION}, ${PLATFORM})..."
curl -fsSL "$REQUEST_URL" -o "$TMP_FILE"

MANIFEST_URL="${BASE_URL}/${PRODUCT}/${RESOLVED_VERSION}/MANIFEST"
if MANIFEST_JSON="$(curl -fsSL "$MANIFEST_URL" 2>/dev/null)"; then
  EXPECTED_SHA="$(printf '%s' "$MANIFEST_JSON" | grep -o '"sha256"[[:space:]]*:[[:space:]]*"[a-f0-9]\{64\}"' | grep -o '[a-f0-9]\{64\}' | head -1 || true)"
  if [[ -n "$EXPECTED_SHA" ]]; then
    ACTUAL_SHA="$(sha256sum "$TMP_FILE" | cut -d' ' -f1)"
    if [[ "$ACTUAL_SHA" != "$EXPECTED_SHA" ]]; then
      echo "error: SHA256 mismatch — expected ${EXPECTED_SHA}, got ${ACTUAL_SHA}" >&2
      exit 1
    fi
    echo "SHA256 verified: ${ACTUAL_SHA}"
  else
    echo "warning: could not parse sha256 from MANIFEST — skipping verification" >&2
  fi
else
  echo "warning: no per-version MANIFEST available — skipping SHA256 verification" >&2
fi

chmod +x "$TMP_FILE"

if [[ -w "$INSTALL_DIR" ]]; then
  mv "$TMP_FILE" "${INSTALL_DIR}/${BINARY_NAME}"
else
  echo "Elevated permission required to write to ${INSTALL_DIR}"
  sudo mv "$TMP_FILE" "${INSTALL_DIR}/${BINARY_NAME}"
fi

echo "Installed ${BINARY_NAME} to ${INSTALL_DIR}/${BINARY_NAME}"
echo "Run '${INSTALL_DIR}/${BINARY_NAME} --help' to get started."
