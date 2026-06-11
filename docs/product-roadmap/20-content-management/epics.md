# Content Management System: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: CMS service route or command, persistence, auth scoped to org/role (via `10`), pagination, and audit events.
- Access: authoring, editorial, and moderation permissions resolve through the `10` role model; no unmoderated content goes live.
- Deterministic: workflow state transitions, taxonomy resolution, and search ranking run without AI; any AI assist (summaries/tags) is suggested, not auto-published.
- Data: content versioned with author and change history; localization variants linked to a canonical item.
- UI: authoring/admin surface and a reader/knowledge-base surface embeddable into `13`.
- Tests: unit (workflow/taxonomy logic), fixture (sample content payloads), API contract, and one failure path (unauthorized publish denied).
- Operations: feature flag, search/index health, and a runbook.

## Category Epics

### EPIC-01: Content Model and Editorial Workflow
- Goal: an author drafts content and an editor publishes it under role control.
- First release: content model (article/guide/post), draft -> review -> publish workflow, and access control via `10`.
- Expansion: scheduled publishing, versioning, and a success-story content type.
- Hardening: audit trail on publish actions and unauthorized-publish tests.

### EPIC-02: Knowledge Base, Search, and Taxonomy
- Goal: growers find relevant best practices quickly.
- First release: a knowledge base with full-text search and categorization/tagging by crop/region/topic.
- Expansion: portal embedding into `13` and localization variants.
- Hardening: search-index health, relevance tuning, and freshness on the embedded surface.

### EPIC-03: Community Contributions and Engagement
- Goal: the community contributes content safely and editors learn what works.
- First release: community contributions through a moderation queue.
- Expansion: content engagement analytics (views/reads/helpfulness).
- Hardening: spam/abuse handling, moderation audit trail, and contributor reputation.
