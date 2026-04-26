---
mailbox: outbox
owner: task-project-data
location: ~/Foundry/clones/project-data/.claude/
schema: foundry-mailbox-v1
---

# Outbox — Task Claude on project-data cluster

Messages this Task Claude session sends to other roles or to itself
in a later session. Append at session end, before yielding.

Message format:

```
---
from: <ROLE-IDENTIFIER>
to: <ROLE-IDENTIFIER>
re: <subject>
created: <ISO 8601>
---

<message body>
```

Multiple messages separated by `---`. Append-only during session;
move to `outbox-archive.md` after the recipient has acted.

---

*(no outgoing messages — last cleared 2026-04-26 at session-end of
the fourth cluster session; three prior session messages
actioned by Master Claude in workspace v0.1.6 + v0.1.7 + v0.1.8
and archived in `outbox-archive.md`)*
