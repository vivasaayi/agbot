# Content Management System: Current State and Target State

## Mission

Be the platform's knowledge surface: a blog and knowledge base that publishes agricultural best practices, educational content, and grower success stories, supports community contribution under moderation, and embeds cleanly into the grower portal.

## Current Maturity

greenfield pending (M0 named): no implementation exists; this domain is a product-vision module from `product-summary.md` (#9 Content Management System). Nothing in the repository implements a content model, editorial workflow, knowledge base, search, moderation, or content analytics.

## What Exists Now

- Nothing is built for this domain. There is no CMS crate, content store, authoring UI, or knowledge base.
- This is the most decoupled domain on the roadmap: a fairly standard CMS, largely independent of the drone/sensor/geospatial stack. It does not consume scenes, layers, telemetry, or geospatial products.
- Adjacent surfaces it would build on (already partially real):
  - Domain `10` (field/farm/data + org/roles): the accounts and role model (admin, editor, contributor, viewer) it gates authoring and moderation through. Itself greenfield-pending, so authoring access is gated on it.
  - Domain `13` (farmers portal): the grower-facing surface the published knowledge base and success stories embed into.

## Gaps to Close

- No content model (article/guide/post) with status, authorship, and versioning.
- No editorial workflow (draft -> review -> publish) or scheduled publishing.
- No knowledge base structure or full-text search.
- No categorization/tagging taxonomy by crop, region, and topic.
- No community contribution path or moderation queue.
- No success-story / case-study content type.
- No localization or multi-language content support.
- No access control mapping CMS roles onto the `10` org/role model.
- No content engagement analytics (views, reads, helpfulness).

## Related Existing Surfaces

- Domain `10` (accounts/roles): the org/role model authoring, editorial, and moderation permissions resolve through.
- Domain `13` (farmers portal): the grower-facing surface the knowledge base and success stories embed into.
- `docs/reference/product-summary.md` (#9 Content Management System): the source description for this module.

## Target Operating Model

- A new CMS crate owns the content model: article/guide/post types with status, authorship, versioning, and a taxonomy by crop/region/topic.
- An editorial workflow moves content draft -> review -> publish, with permissions resolved through the `10` role model; no unmoderated content goes live.
- A knowledge base with full-text search surfaces best practices and success stories, embedded into the portal (`13`) for growers.
- Community contributions flow through a moderation queue before publishing, and localization makes content reachable across regions.
- Content engagement analytics close the loop for editors on what is read and useful.
- The operability pillar leads: this domain's value is a reliable, observable, well-run publishing and search service rather than a geospatial or safety-critical one.
