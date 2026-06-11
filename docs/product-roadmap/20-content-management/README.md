# Content Management System

A blog platform and knowledge base for agricultural best practices, success stories, and educational content, with community-driven contribution and moderation, embedded into the grower portal.

## Where We Are

- Not started / vision only. This is a greenfield product-vision module sourced from `docs/reference/product-summary.md` (#9 Content Management System); no code exists.
- This is the most decoupled domain on the roadmap: it is a fairly standard CMS and is largely independent of the drone, sensor, and geospatial stack. It needs only an accounts/roles model (`10`) and a surface to embed into (`13`).
- The two surfaces it touches are partially real in concept: the grower portal (`13`) it embeds into and the accounts/roles model (`10`) it gates authoring and moderation through — both greenfield-pending.

## Where We Should Be

- A content model (articles, guides, posts) with an editorial workflow takes a draft from author to review to publish, gated by `10` roles.
- A searchable knowledge base, categorized and tagged by crop, region, and topic, surfaces best practices and success stories to growers inside the portal (`13`).
- Community contributions flow through a moderation queue, and localization makes content reachable across regions.
- Content engagement analytics show what is read and useful, closing the loop for editors.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P1 slices.

## Build Order

1. Content model (articles/guides/posts) with authoring, owned via `10` roles.
2. Editorial workflow (draft -> review -> publish) and access control.
3. Knowledge base, search, and categorization/tagging by crop/region/topic.
4. Embedding into the grower portal (`13`) and success-story publishing.
5. Community contributions plus a moderation queue.
6. Localization and content engagement analytics.

## Primary Crates

Planned `content` crate (a CMS service plus an authoring/admin UI; web rendering reused by `13`). Builds on domains `10` (accounts/roles) and `13` (portal surface). Largely independent of `01`-`09`; sequenced after the core drone platform (`01`-`12`) and gated by the advisor MVP, but can run as a near-standalone workstream.
