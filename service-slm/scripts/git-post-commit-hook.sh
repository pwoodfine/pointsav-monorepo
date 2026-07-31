#!/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

# Git post-commit hook — send diff to Doorman /v1/shadow for apprenticeship capture.
# Install: cp service-slm/scripts/git-post-commit-hook.sh .git/hooks/post-commit && chmod +x .git/hooks/post-commit
# Runs asynchronously (&) so it never blocks the commit.
#
# 2026-07-03: restored secret-redaction + diff truncation that the 2026-05-29 swap
# (dacffb1) silently dropped, and restored task_type "shadow-capture" (confirmed via
# git archaeology to be an accidental collapse, not an intentional rename — no prior
# history for "git-commit" anywhere in bin/capture-edit.py's lineage, the sibling copy
# fixed 2026-07-02 in commit 45cfb78). Redaction patterns ported verbatim from that fix.
# This is the canonical template every archive's .git/hooks/post-commit is copied from —
# fixing bin/capture-edit.py alone left this file (and any newly-provisioned archive)
# still vulnerable.
set -euo pipefail

DOORMAN_ENDPOINT="${SLM_DOORMAN_ENDPOINT:-http://127.0.0.1:9080}"

DIFF=$(git diff HEAD~1 HEAD --unified=3 2>/dev/null || git show HEAD --unified=3)

if [ -z "$DIFF" ]; then
    exit 0
fi

COMMIT_MSG=$(git log -1 --pretty=%s 2>/dev/null || echo "git-commit")

PAYLOAD=$(HOOK_DIFF="$DIFF" python3 - "$COMMIT_MSG" <<'PYEOF'
import json, sys, uuid, datetime, os, re

diff_text = os.environ.get('HOOK_DIFF', '')
commit_msg = sys.argv[1] if len(sys.argv) > 1 else "git-commit"
brief_id = uuid.uuid4().hex.upper()
now = datetime.datetime.now(datetime.timezone.utc).isoformat()

DIFF_LINE_LIMIT = 1000

REDACTIONS = [
    (
        re.compile(
            r"-----BEGIN (?:RSA |DSA |EC |OPENSSH |PGP )?PRIVATE KEY-----"
            r".*?"
            r"-----END (?:RSA |DSA |EC |OPENSSH |PGP )?PRIVATE KEY-----",
            re.DOTALL,
        ),
        "[REDACTED PRIVATE KEY]",
    ),
    (re.compile(r"\bAKIA[0-9A-Z]{16}\b"), "[REDACTED AWS KEY]"),
    (re.compile(r"\bsk-(?:proj-)?[A-Za-z0-9_\-]{32,}\b"), "[REDACTED API KEY]"),
    (re.compile(r"\bghp_[A-Za-z0-9]{36,}\b"), "[REDACTED GITHUB TOKEN]"),
    (re.compile(r"\bgho_[A-Za-z0-9]{36,}\b"), "[REDACTED GITHUB OAUTH]"),
    (re.compile(r"\bxox[abprs]-[A-Za-z0-9-]{10,}\b"), "[REDACTED SLACK TOKEN]"),
    (
        re.compile(
            r'(?i)\b(?:bearer|api[_-]?key|secret|token|password)\s*[:=]\s*'
            r'["\']?([A-Za-z0-9/+_\-]{32,})["\']?'
        ),
        lambda m: m.group(0).replace(m.group(1), "[REDACTED]"),
    ),
]


def sanitize(text):
    for pattern, replacement in REDACTIONS:
        text = pattern.sub(replacement, text)
    return text


def truncate_diff(diff):
    lines = diff.split("\n")
    if len(lines) > DIFF_LINE_LIMIT:
        return "\n".join(lines[:DIFF_LINE_LIMIT]) + "\n... [TRUNCATED at {} lines]".format(DIFF_LINE_LIMIT), True
    return diff, False


diff_text, truncated = truncate_diff(diff_text)
diff_text = sanitize(diff_text)
commit_msg = sanitize(commit_msg)

data = {
    "brief": {
        "brief_id": brief_id,
        "created": now,
        "senior_role": "master",
        "senior_identity": "pwoodfine",
        "task_type": "shadow-capture",
        "scope": {"files": []},
        "acceptance_test": "",
        "shadow": True,
        "body": "shadow-capture diff: " + commit_msg
    },
    "actual_diff": diff_text,
    "truncated": truncated
}
print(json.dumps(data))
PYEOF
)

curl -s -X POST "${DOORMAN_ENDPOINT}/v1/shadow" \
    -H "Content-Type: application/json" \
    -H "X-Foundry-Module-ID: git-hook" \
    -d "$PAYLOAD" \
    > /dev/null 2>&1 &
