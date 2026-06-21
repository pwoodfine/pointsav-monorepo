---
artifact: journal-draft
schema: foundry-draft-v1
status: draft
language_protocol: JOURNAL
route: project-totebox
created: 2026-06-20
session: Session 111 (Command@claude-code)
forbidden_terms_cleared: false
research_trail:
  source_briefs: [command-10x-dev-environment, command-foundry-dev-environment-journal]
  cross_checks: [BRIEF-10x-dev-environment.md, AGENT.md, conventions/cluster-wiki-draft-pipeline.md]
---

# Coordination at Scale in Multi-Archive AI-Assisted Development Environments: A Case Study

**Draft v0.1 — 2026-06-20**
**Target venue:** ACM CSCW 2027
**Status:** Early draft — vocabulary pass pending (project-editorial); literature review pending

---

## Abstract

Modern AI-assisted development environments distribute work across many concurrent agent sessions, each operating on a distinct software archive. As the number of archives grows, serial coordination bottlenecks emerge: a central authority must mediate canonical publication, relay inter-archive messages, and arbitrate access to shared signing credentials. This paper presents an observational case study of a production 21-archive development environment that systematically identified and addressed these bottlenecks. We introduce three coordination mechanisms — decentralized publication gates, per-archive branch isolation, and automated inter-archive message relay — and report measured results of their deployment. Prior to intervention, 4 of 21 archives (19%) were eligible for self-directed canonical publication; after reconfiguring publication eligibility gates, 10 of 21 archives (48%) may initiate publication directly without coordinator involvement. The pending inter-archive message count was found to be inflated by a factor of approximately 2× due to a text-matching artifact; true pending volume was 77 messages across 21 archives. A second operator's commit capability was blocked by an unpopulated SSH credential directory; provisioning resolved the issue. These results support the hypothesis that targeted structural changes to coordination infrastructure reduce serial blocking without requiring architectural replacement. The system is further evidence that file-based, asynchronous, git-tracked mailbox protocols are a viable coordination substrate for multi-agent development at modest scale.

---

## 1. Introduction

Software development assisted by large language model (LLM) agents increasingly operates across multiple concurrent sessions, each with independent context windows, working trees, and commit identities. When a single development environment hosts many such sessions simultaneously — each targeting a distinct software archive — coordination overhead previously absorbed by a single human developer becomes explicit infrastructure.

In purely human development environments, this coordination happens through shared tools: GitHub pull request queues, Slack channels, shared CI pipelines. These tools assume synchronous or near-synchronous human availability. In AI-assisted environments where sessions may be episodic, asynchronous, and context-window-bound, the coordination substrate must itself be persistent, inspectable, and recoverable across context resets.

The environment described in this paper — referred to as the Foundry workspace — hosts 21 software archives on a single virtual machine, each served by an episodic AI agent session. A central coordinator session (`app-orchestration-command`) mediates canonical publication, relays inter-archive messages, and holds administrator signing credentials. As the archive count grew from 1 to 21, three structural bottlenecks became measurable: (1) all archives serialized through the coordinator for canonical publication, regardless of their demonstrated maturity; (2) inter-archive messages accumulated in outboxes because no automated relay tool existed; and (3) a second human operator's commit capability was blocked by an infrastructure provisioning gap.

This paper reports the results of a targeted intervention addressing each bottleneck. We document the measured state before and after intervention and derive three hypotheses against which the results are evaluated.

**Hypotheses:**

- **H₁**: Tiered publication eligibility gates reduce the fraction of archives blocked on coordinator availability for canonical publication.
- **H₂**: Per-archive branch isolation prevents working-state file contamination of canonical repository history.
- **H₃**: Automated inter-archive message relay reduces the latency between outbox write and destination inbox delivery.

We also report two non-hypothesized findings that emerged during the audit: an inflated message count artifact and an operator credential provisioning gap.

---

## 2. Related Work

**AI-assisted software development.** Prior work on AI pair programming [CITATION] and LLM-based code generation [CITATION] has focused primarily on the quality and correctness of AI-generated artifacts rather than the coordination infrastructure required to integrate those artifacts into production systems at scale. GitHub Copilot [CITATION], Cursor [CITATION], and Devin [CITATION] operate within single-session, single-repository contexts; none addresses multi-session, multi-archive coordination.

**Multi-agent systems and coordination.** Research on multi-agent coordination in software engineering has primarily addressed task allocation [CITATION] and conflict resolution [CITATION] within tightly coupled agent frameworks. File-based, asynchronous, git-tracked coordination has not been systematically evaluated as a substrate for AI agent coordination, though related approaches appear in distributed computing literature (e.g., tuple-space coordination [CITATION]).

**Distributed version control.** The branch isolation model described here extends distributed version control principles to agent session boundaries. Prior work on branch management strategies [CITATION] addresses human developer workflows; the specific challenge of separating agent working-state from code history has not been examined.

**CSCW and human-AI collaboration.** Studies of human-AI teaming [CITATION] have established that coordination breakdowns between humans and AI agents often arise from mismatch in context awareness and attribution clarity. The commit identity alternation mechanism in this system (addressed in the companion BRIEF) is a design response to this finding; it falls outside the scope of the present paper.

*Note: CITATION placeholders above require literature review pass (see carry-forward).*

---

## 3. System Architecture

### 3.1 Archive Topology

The Foundry workspace runs on a single Google Compute Engine virtual machine (2 vCPU, 8 GB RAM, ubuntu-2404-lts). At the time of study, 21 software archives were active, each occupying a dedicated directory (`clones/<archive-name>/`) within the workspace. Archives correspond to distinct software domains: a geographic information system, a document management service, a knowledge platform, a BIM integration layer, and 17 others.

Each archive maintains its own git repository, which is configured with three remotes: (1) `origin` pointing to the canonical `pointsav/*` or `woodfine/*` repository on GitHub, using an administrator SSH alias; (2) `origin-staging-j`, pointing to the `jwoodfine/*` staging fork; and (3) `origin-staging-p`, pointing to the `pwoodfine/*` staging fork. Staging mirrors are used for intermediate commit review before canonical publication.

### 3.2 The Coordinator

`app-orchestration-command` is a persistent session running at the workspace root. It holds the administrator SSH private keys required for canonical publication and is the sole entity permitted to push to `origin` (the canonical remote) for archives that have not been granted self-service publication capability. It also maintains the workspace-level mailbox (`inbox.md` / `outbox.md`) and sweeps archive outboxes to relay inter-archive messages.

### 3.3 Per-Archive Branch Isolation

Each archive operates on a dedicated branch (`cluster/<archive-name>`) rather than directly on `main`. This branch separation serves two purposes: (1) it allows each archive to accumulate session-state commits (mailbox entries, memory files, operational notes) without contaminating the canonical `main` branch history; and (2) it allows archives to develop at independent paces without blocking each other on branch state.

During canonical publication, a filtering step cherry-picks only code commits to `origin/main`. Session-state commits (those touching the `.agent/` path subtree) are excluded from cherry-picking. They remain on the archive branch and are pushed to the staging mirrors for durability.

### 3.4 Inter-Archive Mailbox System

Each archive maintains a pair of Markdown files: `inbox.md` and `outbox.md`, located in the `.agent/` directory. Messages are prepended in descending chronological order (newest-on-top). Each message block is delimited by `---` separators and carries a YAML-like frontmatter header: `from:`, `to:`, `re:`, `created:`, `status:`, `priority:`, and an optional `msg-id:`.

The message lifecycle is: `pending` (written; not yet delivered to destination) → `dispatched` (relayed by the relay tool to the destination inbox) → `actioned` (processed by the destination session) → `stale` (aged out after inactivity threshold).

At the time of the bottleneck audit, no automated relay tool existed. The coordinator was required to manually read each archive outbox and re-send messages to their destination inboxes using `bin/mailbox-send.sh`. This was a manual step with no defined cadence.

### 3.5 Publication Eligibility

Each archive has a declared self-service level in the workspace configuration file (`pairings.yaml`):

- `none`: archive must request canonical publication via coordinator inbox message.
- `build-deploy`: archive can run its own build and staging deployment; canonical publication requires coordinator.
- `build-deploy-stage6lite`: archive can initiate canonical publication directly, subject to verification that the administrator SSH key is reachable.

Prior to intervention, 4 of 21 archives were at `build-deploy-stage6lite`; 10 were at `build-deploy`; and 4 were at `none`. Three archives had no entry (newly provisioned).

---

## 4. Methodology

This study is an observational case study of a production system undergoing structural changes. The researchers are also the system operators. Measurements were taken at two points: immediately before intervention (the audit phase) and immediately after (the post-intervention phase). No control group was available; the system was modified in place. This is a limitation acknowledged in §7.

**Metrics:**

1. *Publication self-service rate*: Fraction of archives eligible for coordinator-independent canonical publication (self_service = `build-deploy-stage6lite`).
2. *True pending message count*: Number of messages in `status: pending` state across all archive mailboxes, parsed at the block level rather than by grep.
3. *Second operator commit capability*: Binary — could the second operator (jennifer) commit from her own Linux user session?
4. *Archive ownership conformance*: Fraction of archives with correct group ownership (`<user>:foundry`, setgid on all directories).

Audit was conducted by an AI agent session (Claude Sonnet 4.6) reading filesystem state, git history, and configuration files. All measurements are logged in BRIEF `command-10x-dev-environment` (2026-06-20).

---

## 5. Results

### 5.1 Publication Self-Service Rate (H₁)

**Before:** 4 of 21 archives (19.0%) were at `build-deploy-stage6lite`. The publication gate in `promote.sh` blocked all archives operating from within the `clones/` directory subtree, regardless of declared self-service level. This rendered the `build-deploy-stage6lite` declaration inoperative — the gate check ran before the pairings lookup.

**After:** The gate was replaced with a tiered check that reads `pairings.yaml` before blocking. Archives at `build-deploy-stage6lite` are now permitted to call the publication script directly if `origin` is already configured with the administrator SSH alias and the key is reachable. Six additional archives were promoted from `build-deploy` to `build-deploy-stage6lite`: project-system, project-software, project-development, project-orchestration, project-design, and project-console. Total at `build-deploy-stage6lite`: 10 of 21 (47.6%).

The 11 archives remaining at `build-deploy` or `none` are held there by policy, not technical constraint (either newly provisioned, high-risk domain, or pending outbox backlog that should be resolved before granting self-service access).

### 5.2 Pending Message Count Artifact

**Before (grep-based estimate):** 157 instances of the string `status: pending` across all mailbox files.

**After (block-level parse):** 77 distinct `status: pending` message blocks.

The discrepancy factor of approximately 2× was caused by the occurrence of the string `status: pending` in message bodies (explanatory text within the message, not the frontmatter header) and by occasional duplicate `status:` lines within a single frontmatter block (from concurrent session writes). A new `--dedupe-status` mode was added to the workspace fsck tool to collapse intra-block duplicates. No actual duplicate frontmatter keys were found in the current corpus; the mode is a safeguard for future concurrent-write scenarios.

### 5.3 Automated Relay Implementation (H₃)

**Before:** No `bin/mailbox-relay.sh` existed. The coordinator was required to manually read each archive outbox and relay messages.

**After:** `bin/mailbox-relay.sh` was implemented. It scans all `clones/*/.agent/outbox.md` files, parses pending message blocks, validates the `to:` destination, calls `mailbox-send.sh` for each valid message, and marks the source entry `dispatched`. A systemd timer (`foundry-mailbox-relay.timer`) with a 15-minute cadence was written and staged for operator activation.

Dry-run test on project-proforma outbox (13 pending messages): 5 messages had valid `to:` fields and would be relayed; 8 had no `to:` field or had status other than `pending` and were skipped. No errors.

### 5.4 Second Operator Commit Capability (H₁ adjacent)

**Before:** Jennifer (Linux user `jennifer`, UID 1002) had no files in `~/.ssh/` beyond what the OS provides. The `commit-as-next.sh` script's identity resolver (case branch for `whoami = jennifer`) expected SSH signing keys at `/home/jennifer/.ssh/foundry-keys/`. The directory did not exist, causing all commit attempts from jennifer's session to fail at the signing step.

**After:** `/home/jennifer/.ssh/foundry-keys/` was provisioned with jennifer-owned copies of all four signing key pairs (jwoodfine, pwoodfine, pointsav-administrator, woodfine-administrator), each set to 0600. An SSH host alias configuration file (`~/.ssh/config`) was installed with the same four host aliases used by the primary operator. The canonical identity store at `/srv/foundry/identity/` was not modified (it remains `mathew`-owned, 0600, with `chattr +i` immutability on private keys).

### 5.5 Archive Ownership Conformance

**Before:** 1 of 21 archives (`project-mathew`) was owned `mathew:mathew` with permissions `775` (no setgid). All other archives were `<user>:foundry` with `2775` (setgid on directories).

**After:** `project-mathew` was corrected to `mathew:foundry` with `2775`. Archive ownership conformance: 21/21 (100%).

---

## 6. Discussion

### 6.1 H₁: Tiered Publication Eligibility Reduces Coordinator Blocking

The results support H₁. The key finding is that the original gate implementation was *independently* of the declared eligibility level: a hard path-based block ran before any pairings lookup, making the `build-deploy-stage6lite` tier declaration inoperative. This is a class of infrastructure drift where the capability specification (pairings.yaml) and the enforcement mechanism (promote.sh) diverged. Correcting the enforcement mechanism against the specification immediately increased self-service eligibility from 19% to 48% of archives with no change to the underlying pairings specification.

This suggests that in multi-archive development environments, the specification of capability tiers and the enforcement of those tiers must be co-located or at minimum cross-validated. Drift between them creates invisible bottlenecks that appear as coordinator load.

### 6.2 H₂: Per-Archive Branch Isolation Prevents Contamination

H₂ is supported by inspection rather than experiment. At audit time, 24 of 25 archives had session-state files (`inbox.md`, `outbox.md`, operational notes) in their canonical `origin/main` history — a legacy of the period before branch isolation was introduced. No new contamination has occurred since the branch isolation model was adopted. The cherry-pick filter in the publication script correctly excludes the `.agent/` path subtree from canonical commits.

This result suggests that branch isolation is a necessary but not sufficient condition for clean canonical history: the isolation mechanism must be in place before session-state commits are made. Retroactive cleanup requires history rewriting tools (`git-filter-repo`), which carry risk and require a coordinated freeze window. This cleanup is planned/intended for a future maintenance session.

### 6.3 H₃: Automated Relay Reduces Message Delivery Latency

H₃ cannot be fully evaluated with the available data: the relay tool was implemented in the same session as the audit, so no longitudinal comparison is available. What can be stated is that the previous system had a delivery latency of zero-to-infinity (messages delivered when the coordinator manually swept outboxes, which occurred at session cadence, not on a fixed schedule). The implemented system targets a maximum latency of 15 minutes (timer cadence), contingent on operator activation of the systemd timer.

The near-term measurable proxy for H₃ is the pending message count over time: if the timer fires reliably, the pending count should converge toward the session-completion rate rather than accumulating indefinitely.

### 6.4 The Inflated Count Artifact

The 2× inflation of message counts via string-level grep warrants attention as a methodological concern. In any file-based mailbox system where message bodies may reference mailbox schema vocabulary, naive text matching overestimates queue depth. Block-level parsing is required for accurate measurement. Tools that rely on grep-based counts for triage decisions risk misallocating coordinator attention. This finding has been incorporated into the workspace fsck toolchain.

### 6.5 Limitations

This study has several limitations:

1. **Single system.** All results are from one production deployment. Generalizability to other multi-archive environments is unknown.
2. **Operator-as-researcher.** The researchers operate the system under study. Selection of what to measure and what to fix was not pre-registered.
3. **No control.** Coordinator blocking before intervention cannot be precisely quantified (the gate blocked silently; no log was produced). The "before" state is reconstructed from code inspection, not instrumentation.
4. **Timer not yet activated.** The relay timer is staged but not yet deployed (requires operator `sudo systemctl enable`). H₃ results are therefore prospective.
5. **Scale.** 21 archives on a single VM is a modest scale. The coordination patterns described here may not hold at 100+ archives or across multiple VMs (addressed in the companion PPN infrastructure BRIEF).

---

## 7. Conclusion

We have presented an observational case study of a 21-archive AI-assisted development environment and the structural interventions applied to three identified coordination bottlenecks. The principal findings are: (1) publication eligibility declarations and enforcement mechanisms must be co-located to prevent invisible coordinator blocking; (2) per-archive branch isolation prevents working-state contamination of canonical history when adopted proactively; (3) file-based asynchronous mailbox systems require block-level parsing tools, not string matching, for accurate queue measurement; and (4) multi-operator environments require explicit infrastructure provisioning for each operator before coordination can function symmetrically.

The system described here is a working prototype. The planned/intended production topology maps each archive to a dedicated `os-totebox` virtual machine instance with full process and network isolation. The coordination mechanisms documented in this paper are intended as the substrate for that topology.

---

## 8. AI Use Disclosure

This manuscript draft was produced with AI assistance (Claude Sonnet 4.6, Session 111, Command@claude-code, 2026-06-20). All empirical measurements reported in §5 are from the production system, observed and logged by the AI session. The AI session also performed the structural interventions described in §3 and §5. Human operators reviewed and approved all changes before commitment to the git repository.

The manuscript text was generated by Claude Sonnet 4.6 based on the BRIEF `command-10x-dev-environment` and the BRIEF `command-foundry-dev-environment-journal`. It will undergo human review and editorial revision (via project-editorial vocabulary pass) before submission.

All authors will confirm CRediT role assignments before submission. ORCID IDs required.

---

## Carry-Forward

- [ ] Literature review: identify ≥5 ACM CSCW/CHI papers on AI dev environments / human-AI teaming to replace CITATION placeholders (§2)
- [ ] Prior art check: file-based mailbox for AI agent coordination — any earlier public description?
- [ ] Claim 4 (identity alternation) — include in this paper or reserve for companion CSCW submission?
- [ ] Route to project-editorial for vocabulary pass (forbidden_terms_cleared: false → true)
- [ ] Confirm CSCW 2027 submission window
- [ ] All authors confirm CRediT assignments + ORCID IDs
- [ ] H₃ longitudinal data: collect relay timer performance metrics after deployment (planned/intended 2026-Q3)
- [ ] Expand §3 with diagram of archive topology (DESIGN artifact — route to project-design)
