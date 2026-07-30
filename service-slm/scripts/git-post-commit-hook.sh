#!/bin/bash
# Git post-commit hook — send diff to Doorman /v1/shadow for apprenticeship capture.
# Install: cp service-slm/scripts/git-post-commit-hook.sh .git/hooks/post-commit && chmod +x .git/hooks/post-commit
# Runs asynchronously (&) so it never blocks the commit.
set -euo pipefail

DOORMAN_ENDPOINT="${SLM_DOORMAN_ENDPOINT:-http://127.0.0.1:9080}"

DIFF=$(git diff HEAD~1 HEAD --unified=3 2>/dev/null || git show HEAD --unified=3)

if [ -z "$DIFF" ]; then
    exit 0
fi

COMMIT_MSG=$(git log -1 --pretty=%s 2>/dev/null || echo "git-commit")

PAYLOAD=$(HOOK_DIFF="$DIFF" python3 - "$COMMIT_MSG" <<'PYEOF'
import json, sys, uuid, datetime, os

diff_text = os.environ.get('HOOK_DIFF', '')
commit_msg = sys.argv[1] if len(sys.argv) > 1 else "git-commit"
brief_id = uuid.uuid4().hex.upper()
now = datetime.datetime.now(datetime.timezone.utc).isoformat()

data = {
    "brief": {
        "brief_id": brief_id,
        "created": now,
        "senior_role": "master",
        "senior_identity": "pwoodfine",
        "task_type": "git-commit",
        "scope": {"files": []},
        "acceptance_test": "",
        "shadow": True,
        "body": "git-commit diff: " + commit_msg
    },
    "actual_diff": diff_text
}
print(json.dumps(data))
PYEOF
)

curl -s -X POST "${DOORMAN_ENDPOINT}/v1/shadow" \
    -H "Content-Type: application/json" \
    -H "X-Foundry-Module-ID: git-hook" \
    -d "$PAYLOAD" \
    > /dev/null 2>&1 &
