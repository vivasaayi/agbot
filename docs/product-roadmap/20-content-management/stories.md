# Content Management System: Detailed Stories

> Greenfield (M0): no code exists for this domain yet. It is the most decoupled domain on the roadmap — a fairly standard CMS, independent of the drone/sensor/geospatial stack — but is still sequenced after the core platform (`01`–`12`) and the advisor MVP. It depends on the identity/role spine (`10`) for access control and the grower portal (`13`) for embedding. Stories are necessarily coarse and weighted to M1/M2 foundation; everything here is "build from scratch."

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: the inspectable logic — state machines, search indexing, permission resolution — that works without AI. Any AI assist (tag suggestion, summarization) is advisory only and never auto-publishes.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AUTHOR`, `EDITOR`, `PA` platform admin, `GR` grower (reader), `OPS` operator (community contributor).

---

## M1 — Foundation

### STORY 20-01 · M1 · S · P1 — Versioned content model
- **Story**: As `AUTHOR`, I want to create and store a versioned article owned by an author, so that there is a stable, traceable content entity to build on.
- **Deterministic / evidence**: persist `{content_id, type(ARTICLE|GUIDE|POST), author_id, org_id, status, current_version, created_at}` plus an append-only `{version_id, content_id, body, created_at}` history; every edit creates a new version; content owned by one org.
- **Acceptance**:
  - Given an authenticated author, when an article is created and then edited, then both a content row and two version rows exist and the current version points to the latest.
  - Given a request to read content from another org, when it runs, then it is denied (no cross-tenant read).
  - Given a create request with no body, when it runs, then it is rejected with a validation error and no row is written.
- **Tests**: unit (versioning), API contract (create/edit/get/list), authz (cross-tenant read denied), failure path (empty body → rejected).
- **Depends on**: `10` (accounts).

### STORY 20-02 · M1 · M · P2 — Authoring and editorial workflow
- **Story**: As `EDITOR`, I want content to move through a draft → review → publish state machine, so that nothing goes live without review.
- **Deterministic / evidence**: status lifecycle `Draft→InReview→Published` with `Rejected`/`Unpublished` transitions; every transition audited with actor and timestamp; only an editor role may publish; scheduled publish stores a future effective time enforced deterministically.
- **Acceptance**:
  - Given a draft, when an author submits it for review and an editor publishes it, then status is `Published` and each transition is audited with actor and timestamp.
  - Given an author (non-editor), when they attempt to publish, then it is rejected and the content stays `InReview`.
  - Given a publish requested directly from `Draft` (skipping review), when attempted, then it is rejected.
- **Tests**: unit (state machine + scheduled publish), authz (non-editor publish denied), failure path (skip-review publish → rejected).
- **Depends on**: 20-01, 20-03 (roles).

### STORY 20-03 · M1 · S · P2 — Access control via `10` roles
- **Story**: As `PA`, I want CMS editor/contributor/viewer permissions mapped onto the `10` role model, so that there is no unscoped write path to content.
- **Deterministic / evidence**: resolve `{can_author, can_edit, can_publish, can_moderate, can_read}` deterministically from `10` roles within an org; every CMS mutation checks the resolved permission; no permission is granted outside the org.
- **Acceptance**:
  - Given a user with the editor role in `10`, when permissions resolve, then `can_publish` is true and a publish succeeds within their org.
  - Given a viewer-only user, when they attempt any write, then it is denied and audited.
- **Tests**: unit (role→permission mapping), authz (viewer write denied), failure path (cross-org write → denied).
- **Depends on**: `10` (roles), 20-01.

---

## M2 — Captured / Observable

### STORY 20-04 · M2 · M · P2 — Knowledge base and full-text search
- **Story**: As `GR`, I want full-text search over published content, so that I can find best practices and guides quickly.
- **Deterministic / evidence**: index only `Published` content; search returns ranked results deterministically from a query; results respect org/visibility scope; unpublished and other-org content never appears.
- **Acceptance**:
  - Given published articles, when a grower searches a term they contain, then matching items are returned ranked, each linking to the article.
  - Given a draft containing the term, when searched, then it does not appear in results.
  - Given a query with no matches, when searched, then an empty result set is returned (not an error).
- **Tests**: unit (indexing + ranking), integration (only-published indexed), failure path (no-match → empty set).
- **Depends on**: 20-01, 20-02 (published state).

### STORY 20-05 · M2 · S · P2 — Categorization and tagging (crop/region/topic)
- **Story**: As `EDITOR`, I want to tag content by a crop/region/topic taxonomy, so that readers can browse and filter by relevance.
- **Deterministic / evidence**: persist tags from a controlled taxonomy `{crop, region, topic}`; validate tags against the taxonomy; content filterable by tag; any AI tag suggestion is advisory and requires editor confirmation before it attaches.
- **Acceptance**:
  - Given a content item and valid taxonomy tags, when tagged, then the tags persist and the item is retrievable by tag filter.
  - Given an AI-suggested tag, when it is applied, then it only attaches after an editor confirms it (never auto-applied).
  - Given a tag outside the taxonomy, when applied, then it is rejected.
- **Tests**: unit (taxonomy validation), API contract (tag/filter), failure path (off-taxonomy tag → rejected).
- **Depends on**: 20-01.

---

## M3 — Explainable

### STORY 20-06 · M3 · S · P2 — Portal embedding (into `13`)
- **Story**: As `GR`, I want the knowledge base embedded into the grower portal, so that I can read content without leaving the portal.
- **Deterministic / evidence**: the portal (`13`) renders published, org-scoped content through a read-only embed surface; only `Published` content is reachable; embed respects the reader's `10` visibility.
- **Acceptance**:
  - Given published content, when a grower opens the portal knowledge base, then they see the org-scoped published items and can open one.
  - Given an unpublished or other-org item, when accessed via the embed, then it returns `404`/`403` (never leaks).
- **Tests**: integration with `13`, authz (unpublished/other-org not reachable), failure path (direct hit on draft → 404).
- **Depends on**: 20-04 (search/KB), `13` (portal), 20-03 (access control).

### STORY 20-07 · M3 · S · P2 — Content engagement analytics
- **Story**: As `EDITOR`, I want views and reads tracked per published item, so that I can see what content is read and useful.
- **Deterministic / evidence**: persist `{content_id, views, reads, helpful_votes, period}` aggregated deterministically from view/read events; figures scoped per org; helpfulness is an explicit reader signal, not inferred.
- **Acceptance**:
  - Given reader activity on a published item, when analytics aggregate, then per-item views/reads/helpfulness are computed and traceable to events.
  - Given an item with no activity, when analytics run, then it reports zeros (not absent or errored).
- **Tests**: unit (aggregation), API contract (per-item/period), failure path (no activity → zeros).
- **Depends on**: 20-04, 20-06.

### STORY 20-08 · M3 · S · P2 — Success-story / case-study publishing
- **Story**: As `AUTHOR`, I want to publish a structured success-story content type, so that grower outcomes are showcased consistently.
- **Deterministic / evidence**: a success-story type extends the content model with structured fields `{grower, crop, region, outcome_summary, metrics[]}`; passes the same editorial workflow; renders through the same KB/portal surfaces.
- **Acceptance**:
  - Given a success story with required structured fields, when submitted and published, then it persists as the structured type and is searchable/embeddable like other content.
  - Given a success story missing a required structured field, when submitted, then it is rejected with a clear validation error.
- **Tests**: unit (structured-field validation), integration (search/embed reuse), failure path (missing field → rejected).
- **Depends on**: 20-01, 20-02, 20-04.

---

## M4 — Interactive

### STORY 20-09 · M4 · M · P2 — Community contributions and moderation
- **Story**: As `OPS`, I want to submit a community contribution into a moderation queue, so that grower-contributed content is reviewed before going live.
- **Deterministic / evidence**: a contribution persists `Submitted` into a moderation queue; only a moderator role may `Approve`/`Reject`; approved contributions enter the standard publish flow; **no contribution is ever live without clearing moderation**; every decision audited.
- **Acceptance**:
  - Given a community contribution, when submitted, then it lands in the moderation queue as `Submitted` and is not publicly visible.
  - Given a moderator approves it, when approved, then it enters the editorial publish flow with an audited decision.
  - Given a non-moderator attempts to approve, when attempted, then it is rejected and the contribution stays `Submitted`.
- **Tests**: unit (queue state machine), authz (non-moderator approve denied), failure path (unmoderated content never public).
- **Depends on**: 20-02 (workflow), 20-03 (roles).

### STORY 20-10 · M4 · S · P2 — Localization
- **Story**: As `EDITOR`, I want to store and serve a translated version of content, so that content is reachable across regions and languages.
- **Deterministic / evidence**: persist locale-tagged variants `{content_id, locale, version_ref}` linked to a canonical item; serve the requested locale with deterministic fallback to the canonical when a translation is absent; each locale variant carries its own publish status.
- **Acceptance**:
  - Given a content item with a French translation, when requested in French, then the French variant is served.
  - Given a request for a locale with no translation, when served, then it falls back to the canonical locale (never a 500 or empty page).
- **Tests**: unit (locale resolution + fallback), API contract (locale request), failure path (missing locale → canonical fallback).
- **Depends on**: 20-01, 20-02.

---

## Coverage note

This file covers all 10 capabilities in `capability-map.md` with ~10 greenfield stories (≈1 per capability), weighted to M1/M2/M3 with two M4 stories, matching the M1/M2-heavy, mostly-P2 shape of `release-plan.md` (only the content-model slice, 20-01, is P1; no P0; no M5 stories authored since release-plan lists just 2 M5 rows). The curated counts in `release-plan.md` (≈60 rows) expand several of these into sibling slices when implemented (e.g. richer media handling, per-taxonomy browse pages, scheduled-publish edge cases, multi-locale workflow, helpfulness analytics cuts). The operability pillar leads: the value here is a reliable, observable publishing and search service, and any AI assist (tagging, summarization) is advisory only — an editor approves before publish, and no community contribution goes live without clearing moderation.
