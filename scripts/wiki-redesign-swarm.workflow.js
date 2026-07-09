export const meta = {
  name: 'wiki-redesign-swarm',
  description: 'Opus creative research swarm — live audit + research fan-out + adversarial verification + competing ideation + synthesis → Iterations Log block appended to BRIEF-wiki-redesign.md',
  phases: [
    { title: 'Scout', detail: 'Read BRIEF Decisions Locked + Open Questions' },
    { title: 'Audit', detail: 'Live audit of all three wiki instances against institutional design direction', model: 'opus' },
    { title: 'Research', detail: '10 parallel research agents — competitive analysis, typography, UX, institutional precedent', model: 'opus' },
    { title: 'Adversarial', detail: '3 independent refuters — novelty, audience, implementability', model: 'opus' },
    { title: 'Ideation', detail: '3 competing ideators — independent proposals per surface, no cross-contamination', model: 'opus' },
    { title: 'Synthesis', detail: '1 synthesizer — rank concepts, name loop, append Iterations Log to BRIEF', model: 'opus' },
  ],
}

// ── Stable research premise (embedded — does not change between runs) ─────────
//
// Every research/ideation agent receives this as context. Edit this constant
// (not individual agent prompts) when the strategic framing changes.
const RESEARCH_PREMISE = `
CONTEXT — READ THIS FIRST:

We are building a 21st-century record-keeping platform that runs as three separate wikis:
- documentation.pointsav.com — PointSav technical platform documentation; editors: software developers, engineers
- projects.woodfinegroup.com — Development Markets, Architecture, Woodfine Buildings/Development Classes; editors: architects, engineers, construction professionals
- corporate.woodfinegroup.com — Corporate governance, legal records, Ongoing Reporting Requirements; editors: lawyers, accountants, board members

WHAT MAKES THIS DIFFERENT FROM A KNOWLEDGE BASE:
The wikis ARE the authoritative legal/financial record. Lawyers no longer keep Word files in SharePoint — the wiki IS the record. The git history IS the audit trail. The cite page IS the reference standard. The F12 approval gate IS the governance workflow.

DESIGN CONSTRAINT — INSTITUTIONAL, NOT SOFTWARE:
The wikis must look like what they ARE: Ongoing Reporting Requirements.
- DO model on: Financial Times, The Economist, Wikipedia, EDGAR filing interface, corporate annual reports, law reviews, Bloomberg printed research. These look like authoritative institutional documents.
- DO NOT model on: Stripe docs, Linear, Notion, Slack, Q4 Inc., Vercel docs. These look like software.
- Test for every proposal: "Does this feel like reading a document, or using an app?" The former wins.
- Typography and content are 90% of the visual weight. Chrome (nav, controls) should be unobtrusive — present when needed, invisible when reading.

THE COMPETITOR TO BEAT: Q4 Inc. (q4inc.com) — IR communications SaaS (~50% of S&P 500). Their product is how companies TALK TO investors. Our product IS the record itself. Different products. Not competing on IR communications; competing on quality of the authoritative record.

TECHNOLOGY CONSTRAINT:
Rust/axum/maud server — all HTML is generated server-side in Rust code. All CSS is in a single style.css file (~3,500 lines). JavaScript is minimal (one wiki.js file). Every idea must be achievable in CSS + maud markup, with at most minimal vanilla JS. No React. No Tailwind. No external JS frameworks.

PER-INSTANCE EDITOR AUDIENCES:
- corporate.woodfinegroup.com → lawyers, accountants — need lowest-friction, most authoritative aesthetic
- projects.woodfinegroup.com → architects, engineers, construction — need professional reference aesthetic
- documentation.pointsav.com → software developers, engineers — comfortable with technical tools

RESEARCH GOAL: Produce ideas that pass ALL THREE tests:
1. Novel: not currently done by Q4 Inc. or any named competitor
2. Implementable: achievable in CSS + maud markup, minimal vanilla JS
3. Appropriate: sophisticated but obvious once seen; right for lawyers/accountants/executives reading financial/legal content
`.trim()

// ── Schemas ────────────────────────────────────────────────────────────────────

const SCOUT_SCHEMA = {
  type: 'object',
  properties: {
    decisions_locked: { type: 'array', items: { type: 'string' }, description: 'Bullet points from Decisions Locked section' },
    open_questions: { type: 'array', items: { type: 'string' }, description: 'Bullet points from Open Questions section' },
    prior_loop_name: { type: 'string', description: 'Name of most recent loop from Iterations Log, or empty string if none' },
    prior_loop_top_concepts: { type: 'array', items: { type: 'string' }, description: 'Top concepts from most recent loop, or empty if none' },
  },
  required: ['decisions_locked', 'open_questions', 'prior_loop_name', 'prior_loop_top_concepts'],
}

const AUDIT_SCHEMA = {
  type: 'object',
  properties: {
    site: { type: 'string' },
    instance: { type: 'string', enum: ['documentation', 'projects', 'corporate'] },
    visual_quality_score: { type: 'number', description: '1 (marketing-site aesthetic) to 10 (institutional-document aesthetic)' },
    home_layout_description: { type: 'string' },
    nav_structure_description: { type: 'string' },
    broken_or_placeholder_elements: { type: 'array', items: { type: 'string' } },
    strengths: { type: 'array', items: { type: 'string' } },
    gaps_vs_institutional_design: { type: 'array', items: { type: 'string' }, description: 'What makes this look like software/marketing instead of an institutional record?' },
  },
  required: ['site', 'instance', 'visual_quality_score', 'home_layout_description', 'gaps_vs_institutional_design'],
}

const FINDINGS_SCHEMA = {
  type: 'object',
  properties: {
    agent_label: { type: 'string' },
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          observation: { type: 'string' },
          source: { type: 'string', description: 'Site name, publication, or named design system' },
          relevance: { type: 'string', enum: ['high', 'medium', 'low'] },
        },
        required: ['observation', 'source', 'relevance'],
      },
    },
    novel_opportunities: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          idea: { type: 'string', description: 'One-sentence description of the design idea' },
          surface: { type: 'string', enum: ['main_page', 'article', 'header_footer'] },
          feasibility: { type: 'string', enum: ['high', 'medium', 'low'], description: 'Can this be done in CSS + maud + minimal vanilla JS?' },
          why_not_already_done: { type: 'string', description: 'Why Q4 Inc. / competitors have not done this' },
        },
        required: ['idea', 'surface', 'feasibility'],
      },
    },
    refuted_assumptions: { type: 'array', items: { type: 'string' }, description: 'Beliefs about the design space this research disproves' },
  },
  required: ['agent_label', 'findings', 'novel_opportunities', 'refuted_assumptions'],
}

const ADVERSARIAL_SCHEMA = {
  type: 'object',
  properties: {
    adversarial_lens: { type: 'string', enum: ['novelty', 'audience', 'implementability'] },
    verdicts: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          idea_summary: { type: 'string' },
          verdict: { type: 'string', enum: ['confirmed', 'refuted', 'partial'] },
          reason: { type: 'string' },
        },
        required: ['idea_summary', 'verdict', 'reason'],
      },
    },
    surviving_ideas: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          idea: { type: 'string' },
          surface: { type: 'string', enum: ['main_page', 'article', 'header_footer'] },
          confidence: { type: 'string', enum: ['high', 'medium'] },
        },
        required: ['idea', 'surface', 'confidence'],
      },
    },
  },
  required: ['adversarial_lens', 'verdicts', 'surviving_ideas'],
}

const IDEATION_SCHEMA = {
  type: 'object',
  properties: {
    agent_name: { type: 'string' },
    lens: { type: 'string' },
    main_page: {
      type: 'array', minItems: 3, maxItems: 3,
      items: {
        type: 'object',
        properties: {
          title: { type: 'string' },
          description: { type: 'string' },
          why_novel: { type: 'string' },
          css_mechanism: { type: 'string', description: 'Which CSS properties/features implement this — grid, custom-property, @container, etc.' },
          implementation_notes: { type: 'string' },
        },
        required: ['title', 'description', 'why_novel', 'css_mechanism'],
      },
    },
    article: {
      type: 'array', minItems: 3, maxItems: 3,
      items: {
        type: 'object',
        properties: {
          title: { type: 'string' },
          description: { type: 'string' },
          why_novel: { type: 'string' },
          css_mechanism: { type: 'string' },
          implementation_notes: { type: 'string' },
        },
        required: ['title', 'description', 'why_novel', 'css_mechanism'],
      },
    },
    header_footer: {
      type: 'array', minItems: 3, maxItems: 3,
      items: {
        type: 'object',
        properties: {
          title: { type: 'string' },
          description: { type: 'string' },
          why_novel: { type: 'string' },
          css_mechanism: { type: 'string' },
          implementation_notes: { type: 'string' },
        },
        required: ['title', 'description', 'why_novel', 'css_mechanism'],
      },
    },
  },
  required: ['agent_name', 'lens', 'main_page', 'article', 'header_footer'],
}

// ── Phase 0 — Scout ─────────────────────────────────────────────────────────

phase('Scout')
log('Reading BRIEF context — Decisions Locked, Open Questions, prior loop')

const scout = await agent(
  `Read the file at /srv/foundry/clones/project-knowledge/.agent/briefs/BRIEF-wiki-redesign.md.

Extract and return as structured output:
1. The full content of the "## Decisions Locked" section (one bullet per array item)
2. The full content of the "## Open Questions" section (one bullet per array item)
3. The name of the most recent Iterations Log loop (e.g. "Loop Cerulean Compass") — empty string if no loops yet
4. The top-ranked concepts from that most recent loop — empty array if no loops yet

Return ONLY what is in the file. Do not invent or paraphrase beyond extracting.`,
  { schema: SCOUT_SCHEMA, label: 'brief-scout', model: 'opus' }
)

const decisionsLocked = scout ? scout.decisions_locked.join('\n- ') : '(none yet)'
const openQuestions = scout ? scout.open_questions.join('\n- ') : '(none yet)'
const priorLoopName = scout ? scout.prior_loop_name : ''
const priorTopConcepts = scout && scout.prior_loop_top_concepts.length > 0
  ? 'Prior loop top concepts (DO NOT repeat these — go deeper or find new ground):\n' + scout.prior_loop_top_concepts.join('\n')
  : '(first run — no prior concepts to avoid)'

// ── Phase 1 — Live Audit ─────────────────────────────────────────────────────

phase('Audit')
log('Auditing all three wiki instances against institutional design direction')

const makeAuditPrompt = (url, instance) => `${RESEARCH_PREMISE}

YOUR TASK: Audit this wiki instance against the institutional design direction above.

Instance: ${instance}
URL: ${url}

Steps:
1. Fetch ${url} using the WebFetch tool and examine the HTML/rendered content
2. Also fetch a sample article page (try ${url}wiki/index if it exists, or any article link you find)
3. Evaluate against the institutional-document design standard

Specific things to look for:
- Does the page look like a corporate filing database or like a marketing site?
- Typography: does it feel like Financial Times / law review, or like a SaaS product?
- Chrome (nav, footer): is it unobtrusive institutional, or prominent UI?
- Color: restrained and document-like, or branded and software-like?
- Spacing and density: print-quality, or web-app airy?
- Any broken links, placeholder elements (href="#"), or unimplemented stubs?

Return your findings as structured output.`

const audits = (await parallel([
  () => agent(makeAuditPrompt('https://documentation.pointsav.com/', 'documentation'), { schema: AUDIT_SCHEMA, label: 'audit-documentation', model: 'opus', phase: 'Audit' }),
  () => agent(makeAuditPrompt('https://projects.woodfinegroup.com/', 'projects'), { schema: AUDIT_SCHEMA, label: 'audit-projects', model: 'opus', phase: 'Audit' }),
  () => agent(makeAuditPrompt('https://corporate.woodfinegroup.com/', 'corporate'), { schema: AUDIT_SCHEMA, label: 'audit-corporate', model: 'opus', phase: 'Audit' }),
])).filter(Boolean)

const auditSummary = audits.map(a =>
  `${a.instance} (${a.site}): quality ${a.visual_quality_score}/10. Gaps: ${(a.gaps_vs_institutional_design || []).join('; ')}`
).join('\n')

log(`Audit complete. Scores: ${audits.map(a => `${a.instance}=${a.visual_quality_score}/10`).join(', ')}`)

// ── Phase 2 — Research Fan-Out ───────────────────────────────────────────────

phase('Research')
log('Dispatching 10 research agents in parallel')

const makeResearchContext = (agentLabel, focus, keyQuestion) => `${RESEARCH_PREMISE}

DECISIONS ALREADY LOCKED (treat these as hard constraints, not questions):
- ${decisionsLocked}

OPEN QUESTIONS TO PRIORITIZE (this is where the swarm should find answers):
- ${openQuestions}

${priorTopConcepts}

LIVE AUDIT RESULTS (current state of the three wikis):
${auditSummary}

YOUR FOCUS: ${focus}
YOUR KEY QUESTION: ${keyQuestion}

Research this focus area thoroughly. Use WebFetch, WebSearch, or direct URL access to examine real sites.
For every finding: cite the specific site or source you examined.
For every novel opportunity: be specific about which surface (main_page, article, header_footer) and how it would be implemented in CSS + maud.
Flag any assumption you find that is wrong — "refuted_assumptions" is important.

Return structured output with your findings.`

const RESEARCH_AGENTS = [
  {
    label: 'q4-audit',
    focus: 'Deep audit of Q4 Inc. (q4inc.com) — the named competitor',
    question: 'What does Q4 Inc. NOT do that we could do first and better? What are their documented user complaints?',
  },
  {
    label: 'award-sites',
    focus: 'Awwwards, CSSDA, and SiteInspire winners 2024-2025 in the financial, legal, and institutional categories',
    question: 'What 3-5 design patterns appear consistently in best-in-class institutional sites that Q4 Inc. and typical IR platforms do NOT use?',
  },
  {
    label: 'financial-press',
    focus: 'Financial Times (ft.com), Bloomberg, Reuters, The Economist — their editorial and typographic patterns',
    question: 'How do the best financial publishers structure ongoing corporate information? What typographic and layout choices convey authority?',
  },
  {
    label: 'wikipedia-analysis',
    focus: 'Well-developed Wikipedia articles — specifically company, legal, and financial pages',
    question: 'What makes the best Wikipedia articles feel like an authoritative reference work rather than a website? What specifically produces that sensation?',
  },
  {
    label: 'records-management',
    focus: 'Legal document management systems: iManage, NetDocuments, Clio, and court filing portals',
    question: 'What features do lawyers expect in a records system? What would make them genuinely prefer a wiki-based record over their current file storage?',
  },
  {
    label: 'typography-research',
    focus: 'Font pairings specifically for financial/legal institutional content — NOT tech/startup fonts',
    question: 'Which font combinations convey authority + readability for lawyers, accountants, and executives? Cite specific typefaces and why they work for this audience.',
  },
  {
    label: 'print-layout',
    focus: 'Print-inspired CSS layout patterns — grid systems, margins, reading column width, vertical rhythm',
    question: 'What CSS techniques produce a print-quality reading experience on screen? Specifically: correct measure (column width), baseline grid, margin proportions for legal/financial documents.',
  },
  {
    label: 'institutional-precedent',
    focus: 'Government and regulatory filing databases: EDGAR (sec.gov/cgi-bin/browse-edgar), SEDAR, UK Companies House, ASX disclosure portal',
    question: 'What is the actual visual and UX pattern of EDGAR and SEDAR? What makes them feel authoritative despite (or because of) their austere design?',
  },
  {
    label: 'mobile-exec',
    focus: 'How C-suite executives and board members consume corporate documents on mobile',
    question: 'What reading patterns do lawyers, accountants, and board members actually use on mobile? What makes a financial document readable on an iPhone at arm\'s length in a boardroom?',
  },
  {
    label: 'competitor-gaps',
    focus: 'Three additional competitors beyond Q4 Inc.: Notified (formerly GlobeNewswire), Broadridge, and Computershare communications platforms',
    question: 'What documented user frustrations exist with these platforms? What features are missing that a records-first platform could uniquely provide?',
  },
]

const researchResults = (await parallel(
  RESEARCH_AGENTS.map(({ label, focus, question }) => () =>
    agent(makeResearchContext(label, focus, question), { schema: FINDINGS_SCHEMA, label, model: 'opus', phase: 'Research' })
  )
)).filter(Boolean)

const allOpportunities = researchResults.flatMap(r => r.novel_opportunities || [])
const allFindings = researchResults.flatMap(r => r.findings || [])
log(`Research complete. ${allOpportunities.length} novel opportunities surfaced across ${researchResults.length} agents`)

// ── Phase 3 — Adversarial Verification ──────────────────────────────────────

phase('Adversarial')
log('Running 3 adversarial refuters against all research findings')

const opportunitiesSummary = allOpportunities
  .map(o => `[${o.surface}] ${o.idea} (feasibility: ${o.feasibility})`)
  .join('\n')

const makeAdversarialPrompt = (lens, instruction) => `${RESEARCH_PREMISE}

YOUR ROLE: Adversarial refuter. You are trying to DISPROVE the research findings below.
Default to refuted=true if uncertain — only confirm ideas that clearly survive scrutiny.

ADVERSARIAL LENS: ${lens}
YOUR JOB: ${instruction}

ALL NOVEL OPPORTUNITIES FROM RESEARCH PHASE (your targets):
${opportunitiesSummary}

For each idea, decide: confirmed (survives this lens), refuted (fails this lens), or partial (survives with modifications).
Only include ideas in surviving_ideas if you are confident they pass your specific lens.
Be harsh. False positives (confirming bad ideas) are worse than false negatives.`

const adversarialResults = (await parallel([
  () => agent(
    makeAdversarialPrompt('novelty', 'Has this been done before? Find an existing site already doing exactly this. If you can find a real example, refute it. "Novel" means genuinely unprecedented in the financial/legal/institutional space.'),
    { schema: ADVERSARIAL_SCHEMA, label: 'refute-novelty', model: 'opus', phase: 'Adversarial' }
  ),
  () => agent(
    makeAdversarialPrompt('audience', 'Would a lawyer, accountant, or board member ACTUALLY use this? Apply scepticism: is this for designers, or for the actual audience (legal/financial professionals reading on deadline in a boardroom)?'),
    { schema: ADVERSARIAL_SCHEMA, label: 'refute-audience', model: 'opus', phase: 'Adversarial' }
  ),
  () => agent(
    makeAdversarialPrompt('implementability', 'Can this be built in CSS + maud markup + minimal vanilla JS with no React/Tailwind/external framework? Be specific: name the CSS property or feature required. If it needs a JS framework or server-side logic beyond a template, refute it.'),
    { schema: ADVERSARIAL_SCHEMA, label: 'refute-implementability', model: 'opus', phase: 'Adversarial' }
  ),
])).filter(Boolean)

// Idea survives if confirmed or partial by majority (2 of 3 lenses)
const ideaVotes = {}
for (const result of adversarialResults) {
  for (const v of (result.verdicts || [])) {
    const key = v.idea_summary
    if (!ideaVotes[key]) ideaVotes[key] = { confirmed: 0, refuted: 0, partial: 0 }
    ideaVotes[key][v.verdict]++
  }
}
const survivingIdeas = adversarialResults.flatMap(r => r.surviving_ideas || [])
const uniqueSurviving = []
const seen = new Set()
for (const idea of survivingIdeas) {
  const key = idea.idea.slice(0, 60)
  if (!seen.has(key)) { seen.add(key); uniqueSurviving.push(idea) }
}

const confirmedSummary = uniqueSurviving
  .map(i => `[${i.surface}] ${i.idea}`)
  .join('\n')

log(`Adversarial complete. ${uniqueSurviving.length} ideas survived all three refuters`)

// ── Phase 4 — Competing Ideation ─────────────────────────────────────────────

phase('Ideation')
log('3 independent ideators proposing — no cross-contamination')

const makeIdeationPrompt = (agentName, lens, lensInstruction) => `${RESEARCH_PREMISE}

DECISIONS ALREADY LOCKED (hard constraints for your proposals):
- ${decisionsLocked}

CONFIRMED RESEARCH FINDINGS (ideas that survived adversarial review — use these as inspiration):
${confirmedSummary || '(first run — no confirmed findings yet; use the research premise and your own judgment)'}

YOUR IDENTITY: You are ideation agent "${agentName}", working through the "${lens}" lens.
${lensInstruction}

YOUR TASK: Propose exactly 3 novel design ideas for EACH of these three surfaces:
1. Main page (the home page that records-keepers land on)
2. Article page (the document reading experience — typography, layout, TOC, metadata)
3. Header and footer (the persistent chrome across all pages)

Total: 9 proposals (3 per surface).

CONSTRAINTS FOR ALL PROPOSALS:
- Novel: not done by Q4 Inc. or the named competitors; not already in Decisions Locked
- Implementable: achievable in CSS + maud markup, minimal vanilla JS (name the CSS mechanism)
- Appropriate: sophisticated but obvious once seen; right for lawyers/accountants/executives
- Institutional: passes the "document, not app" test

DO NOT look at other ideation agents' proposals — you are working independently.
DO NOT propose incremental improvements to existing patterns — propose the unexpected.`

const ideationResults = (await parallel([
  () => agent(
    makeIdeationPrompt(
      'Alpha',
      'cognitive load reduction',
      'Your lens is cognitive load: what design choices reduce the mental effort of finding, reading, and citing authoritative records? Think about progressive disclosure, information hierarchy, and how professional readers (lawyers on deadline) scan documents.'
    ),
    { schema: IDEATION_SCHEMA, label: 'ideate-alpha', model: 'opus', phase: 'Ideation' }
  ),
  () => agent(
    makeIdeationPrompt(
      'Beta',
      'institutional authority signals',
      'Your lens is institutional authority: what visual and structural signals make a document feel like an authoritative legal/financial record rather than a website? Think about how courts, regulators, and auditors perceive documents. What makes EDGAR feel like EDGAR?'
    ),
    { schema: IDEATION_SCHEMA, label: 'ideate-beta', model: 'opus', phase: 'Ideation' }
  ),
  () => agent(
    makeIdeationPrompt(
      'Gamma',
      'information architecture',
      'Your lens is information architecture: how should a records-keeping wiki be organized, navigated, and cross-referenced? Think about how law libraries, government archives, and financial databases structure information for professional retrieval, citation, and audit.'
    ),
    { schema: IDEATION_SCHEMA, label: 'ideate-gamma', model: 'opus', phase: 'Ideation' }
  ),
])).filter(Boolean)

log(`Ideation complete. ${ideationResults.reduce((n, r) => n + r.main_page.length + r.article.length + r.header_footer.length, 0)} total proposals across ${ideationResults.length} ideators`)

// ── Phase 5 — Synthesis ──────────────────────────────────────────────────────

phase('Synthesis')
log('Synthesizing all proposals into ranked Iterations Log block')

const allProposalsSummary = ideationResults.map(r => `
IDEATOR ${r.agent_name} (lens: ${r.lens}):
Main page:
${r.main_page.map((p, i) => `  M${i+1}. ${p.title}: ${p.description} [CSS: ${p.css_mechanism}]`).join('\n')}
Article:
${r.article.map((p, i) => `  A${i+1}. ${p.title}: ${p.description} [CSS: ${p.css_mechanism}]`).join('\n')}
Header/footer:
${r.header_footer.map((p, i) => `  H${i+1}. ${p.title}: ${p.description} [CSS: ${p.css_mechanism}]`).join('\n')}
`).join('\n---\n')

const auditScores = audits.map(a => `${a.instance}: ${a.visual_quality_score}/10`).join(', ')

const synthesisPromptText = `${RESEARCH_PREMISE}

YOUR ROLE: Synthesizer. You read all ideation proposals, rank them, name this loop, and write the Iterations Log block to the BRIEF.

AUDIT BASELINE: ${auditScores}

ALL IDEATION PROPOSALS:
${allProposalsSummary}

CONFIRMED RESEARCH FINDINGS (survived adversarial):
${confirmedSummary || '(none confirmed this run)'}

${priorLoopName ? `PRIOR LOOP NAME: "${priorLoopName}" — choose a DIFFERENT two-word colour-noun for this loop.` : 'This is the first loop. Choose a two-word colour-noun name (e.g. "Cerulean Compass", "Amber Ledger", "Indigo Seal").'}

STEP 1 — Rank. For each surface (main_page, article, header_footer), pick the top 3 proposals from ALL ideators combined. Rank by: (1) novelty, (2) institutional appropriateness, (3) implementability. Cross-ideator is fine — pick the best regardless of source. You may also graft the second-best idea from a runner-up onto the winner.

STEP 2 — Name this loop. Pick a two-word colour-noun (not "${priorLoopName || 'any prior name'}").

STEP 3 — Write the Iterations Log block. Use EXACTLY this format:

\`\`\`
### Loop [Name] — [YYYY-MM-DD]
**Model:** claude-opus-4-8
**Audit baseline:** documentation X/10, projects X/10, corporate X/10
**Gaps closed vs. prior loop:** [list gaps closed, or "first run"]
**Regressions detected:** [list or "none"]
**Research findings (survived adversarial):** [bullet list of top confirmed findings]
**Top-ranked concepts:**
  - Main page M-1: [title] — [one-line description]
  - Main page M-2: [title] — [one-line description]
  - Main page M-3: [title] — [one-line description]
  - Article A-1: [title] — [one-line description]
  - Article A-2: [title] — [one-line description]
  - Article A-3: [title] — [one-line description]
  - Header/Footer H-1: [title] — [one-line description]
  - Header/Footer H-2: [title] — [one-line description]
  - Header/Footer H-3: [title] — [one-line description]
**Implemented this loop:** none
**Carry to next loop:** [what to go deeper on next session]
\`\`\`

STEP 4 — Append to BRIEF. Read the file at:
/srv/foundry/clones/project-knowledge/.agent/briefs/BRIEF-wiki-redesign.md

Find the line that reads "## Iterations Log" and the descriptive paragraph after it.
Insert your new loop block AFTER the descriptive paragraph and BEFORE any existing loop blocks.
Use the Edit tool to insert the block. The block must start with "### Loop" and end with the "Carry to next loop" line.

After editing the BRIEF, return the loop name and the top 3 concepts per surface.`

const synthesis = await agent(synthesisPromptText, { label: 'synthesizer', model: 'opus', phase: 'Synthesis' })

log(`Synthesis complete. Loop block appended to BRIEF-wiki-redesign.md`)
return synthesis
