#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
    echo "[ERROR] Usage: ./cognitive-bridge.sh <TOTEBOX_ROOT>"
    exit 1
fi

TOTEBOX_ROOT="$1"

IN_QUEUE="$TOTEBOX_ROOT/service-slm/transient-queues"
OUT_QUEUE="$TOTEBOX_ROOT/knowledge-graph"
ARCHIVE_DIR="$TOTEBOX_ROOT/service-slm/processed-payloads"

# Ensure physical boundaries exist
mkdir -p "$IN_QUEUE" "$OUT_QUEUE" "$ARCHIVE_DIR"

echo "[SYSTEM] Cognitive Bridge armed. Watching: $IN_QUEUE"
echo "--------------------------------------------------------"

while true; do
    for TXT_FILE in "$IN_QUEUE"/*.txt; do
        if [ -f "$TXT_FILE" ]; then
            FILENAME=$(basename "$TXT_FILE")
            TX_ID=$(echo "$FILENAME" | grep -o 'TX-[A-Z0-9]*')
            echo "[DETECTED] Staging Payload: $FILENAME"
            
            # 1. Read the YAML Frontmatter & Body
            PAYLOAD_CONTENT=$(cat "$TXT_FILE")

            # 2. Route through the Doorman at SLM_BIND_ADDR (default 127.0.0.1:9080)
            # Per service-slm/ARCHITECTURE.md: Doorman is the boundary, not raw LLM.
            SLM_ENDPOINT="${SLM_BIND_ADDR:-127.0.0.1:9080}"

            echo "[SYSTEM] Routing through Doorman at http://$SLM_ENDPOINT..."

            # Construct request JSON with payload content as user message.
            # Module ID 'foundry' identifies this as internal bridge traffic.
            REQUEST_JSON=$(cat <<EOF
{
  "model": "olmo-3-7b-instruct",
  "messages": [
    {"role": "user", "content": "$PAYLOAD_CONTENT"}
  ],
  "max_tokens": 1024,
  "temperature": 0.7
}
EOF
)

            # POST to Doorman /v1/chat/completions with Foundry module ID
            RESPONSE=$(curl -s -X POST "http://$SLM_ENDPOINT/v1/chat/completions" \
              -H "Content-Type: application/json" \
              -H "X-Foundry-Module-ID: foundry" \
              -d "$REQUEST_JSON")

            # Extract content from response; handle curl errors gracefully
            if [ $? -ne 0 ]; then
              echo "[ERROR] Doorman request failed for $FILENAME"
              continue
            fi

            # Parse the response content (expected: OpenAI-compatible format)
            CONTENT=$(echo "$RESPONSE" | jq -r '.choices[0].message.content // "[ERROR] No content in response"' 2>/dev/null)
            if [ -z "$CONTENT" ] || [ "$CONTENT" = "[ERROR] No content in response" ]; then
              echo "[WARNING] Empty or malformed Doorman response for $FILENAME"
              CONTENT="[ERROR] Doorman returned empty response"
            fi

            RESPONSE="$CONTENT"
            
            # 3. Forge the Markdown Overlay from Doorman response
            OUT_FILE="$OUT_QUEUE/${TX_ID}_overlay.md"
            echo -e "$RESPONSE" > "$OUT_FILE"

            echo "[ROUTED] Overlay from Doorman -> $OUT_FILE"
            
            # 4. Vault the processed payload to prevent loops
            mv "$TXT_FILE" "$ARCHIVE_DIR/"
            echo "--------------------------------------------------------"
        fi
    done
    sleep 5
done
