# Farmers Portal: Detailed Stories

> Greenfield domain (M0 named): no code exists yet. Every story below is **built from scratch** and is gated behind the core drone platform (`01`–`12`) and the advisor MVP (`09`). This is a presentation/consumption surface, so stories are coarser, weighted to M1 foundation, and almost entirely P2 (only grower identity is P1).

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `GR` grower, `AG` agronomist, `PA` platform admin, `OPS` operator.

---

## M1 — Foundation

### STORY 13-01 · M1 · M · P1 — Grower account and org/role access
- **Story**: As `GR`, I want to sign in and have the portal resolve my organization, role, farms, and fields through the `10` spine, so that every view I see is mine and tenant-safe.
- **Deterministic / evidence**: resolve `{grower_id, org_id, role, farm_ids[], field_ids[]}` from the `10` access model on session start; every subsequent read carries the resolved scope; no query may return an entity outside it.
- **Acceptance**:
  - Given a grower in org A, when they sign in, then their session resolves only org-A farms/fields from `10`.
  - Given a grower scoped to org A, when they request a field owned by org B, then the request is denied (403) and audited — no cross-tenant leak.
- **Tests**: API contract (sign-in/scope), authz (cross-tenant denied), unit (scope resolution).
- **Depends on**: `10` (org/role/field model).

### STORY 13-02 · M1 · M · P2 — Grower dashboard (home)
- **Story**: As `GR`, I want a home screen listing my farms and fields with their latest activity, so that I have one front door to my operation.
- **Deterministic / evidence**: render home strictly from `10` farms/fields the session resolved; each card shows `{field_id, last_scene_date|none, open_recommendation_count}` computed from `09`/`07` read-only.
- **Acceptance**:
  - Given a grower with two farms, when they open home, then both farms and their fields render with last-activity summaries.
  - Given a grower with no fields yet, when they open home, then an explicit empty state renders (not an error or a blank page).
- **Tests**: UI (render), fixture (seeded farms/fields), failure path (empty portfolio).
- **Depends on**: 13-01, `10`.

### STORY 13-03 · M1 · M · P2 — Field and farm overview
- **Story**: As `GR`, I want a per-field overview that aggregates the latest scene, layer, and finding, so that I can judge a field's status without opening every report.
- **Deterministic / evidence**: aggregate read-only from `07`/`09`/`10`: latest scene date, layer availability, and most recent finding/recommendation per field; portal owns no field data.
- **Acceptance**:
  - Given a field with a completed analysis, when its overview opens, then the latest scene, available layers, and newest finding are shown with their source dates.
  - Given a field whose latest scene failed to process, when the overview opens, then it shows "no current analysis" rather than stale or fabricated findings.
- **Tests**: API contract (aggregation), fixture (multi-date field), failure path (no usable scene).
- **Depends on**: 13-01, `07`, `09`, `10`.

### STORY 13-04 · M1 · S · P2 — Saved views and preferences
- **Story**: As `GR`, I want to persist a default farm/field view and basic preferences, so that the portal opens where I work.
- **Deterministic / evidence**: persist `{grower_id, default_farm_id, default_field_id, units, prefs}`; preferences are scoped per grower and never alter another grower's view.
- **Acceptance**:
  - Given a grower sets a default field, when they next sign in, then the portal opens on that field.
  - Given a default field that is later removed from the grower's scope, when they sign in, then the portal falls back to home with a notice (no broken/forbidden view).
- **Tests**: API contract (save/load), unit (fallback), failure path (default out of scope).
- **Depends on**: 13-01.

---

## M2 — Captured / Observable

### STORY 13-05 · M2 · M · P2 — Report inbox
- **Story**: As `GR`, I want an inbox that lists and opens the advisor reports for my fields, so that I can read what each flight found without logging into the agronomist tools.
- **Deterministic / evidence**: list `09` reports filtered to the session's field scope; each row carries `{report_id, field_id, generated_at, status}`; opening renders the `09` report read-only, portal stores no report content of its own.
- **Acceptance**:
  - Given completed reports for the grower's fields, when the inbox opens, then they list newest-first, paginated, and filterable by field/date.
  - Given a report belonging to another org, when the grower requests it by ID, then access is denied and audited.
- **Tests**: API contract (list/pagination/filter), authz (cross-tenant report denied), fixture (seeded reports).
- **Depends on**: 13-01, `09` (reports), `10`.

### STORY 13-06 · M2 · M · P2 — Recommendation tracking
- **Story**: As `GR`, I want to see each recommendation from a report and acknowledge it, so that I can track what I still need to act on.
- **Deterministic / evidence**: surface `09` recommendations with `{recommendation_id, field_id, priority, status}`; an acknowledgement writes an audited status transition (`open→acknowledged`) back through the owning model, never a portal-local copy.
- **Acceptance**:
  - Given an open recommendation, when the grower acknowledges it, then its status becomes `acknowledged` with actor and timestamp recorded.
  - Given a recommendation the grower does not own, when they attempt a status change, then it is rejected and audited.
- **Tests**: API contract (transition), audit (actor/timestamp), failure path (unauthorized transition).
- **Depends on**: 13-05, `09` (recommendation entity), `10`.

### STORY 13-07 · M2 · M · P2 — Field map and layer view
- **Story**: As `GR`, I want a read-only field map with one GIS layer overlaid, so that I can see where a finding is on my field.
- **Deterministic / evidence**: render the field boundary and one `07` layer; assert the layer's CRS/extent match the field before overlay; reuse `08` rendering patterns. No edit/annotation in the portal.
- **Acceptance**:
  - Given a field with an NDVI layer, when the map opens, then the boundary and layer align in the correct CRS/extent.
  - Given a layer whose CRS/extent does not match the field, when overlay is requested, then it is refused with an explicit mismatch error (no misaligned overlay shown).
- **Tests**: geospatial round-trip (CRS/extent), UI (overlay), failure path (CRS mismatch).
- **Depends on**: 13-03, `07`, `08`, `10`.

---

## M3 — Explainable

### STORY 13-08 · M3 · S · P2 — Notifications and alert feed
- **Story**: As `GR`, I want a feed that notifies me when a new report or recommendation lands on one of my fields, so that I act without polling the inbox.
- **Deterministic / evidence**: a notification is generated from a concrete source event `{event_type, source_ref, field_id, created_at}`; the feed only lists events for fields in the grower's scope; later wired to `15`/`17` risk events.
- **Acceptance**:
  - Given a new report for an owned field, when it is published by `09`, then a notification appears in the grower's feed citing the report.
  - Given an event on a field outside the grower's scope, when it fires, then no notification is delivered to that grower.
- **Tests**: integration (event→feed), authz (scope filter), failure path (out-of-scope event suppressed).
- **Depends on**: 13-01, 13-05, `09`.

### STORY 13-09 · M3 · S · P2 — Recommendation status history
- **Story**: As `GR`, I want each recommendation to show its full status history, so that I can prove what was done and when.
- **Deterministic / evidence**: render the audited transition log `{from, to, actor, timestamp}` for a recommendation, sourced from the owning audit trail; portal renders, does not author, history.
- **Acceptance**:
  - Given a recommendation moved open→acknowledged→done, when its history opens, then all three transitions render in order with actor and timestamp.
  - Given a recommendation with no transitions yet, when history opens, then an explicit "no activity" state renders.
- **Tests**: API contract (history), fixture (multi-transition), failure path (empty history).
- **Depends on**: 13-06.

---

## M4 — Interactive

### STORY 13-10 · M4 · M · P2 — Data export and sharing
- **Story**: As `GR`, I want to export a grower-scoped field summary and share it, so that I can hand my data to an advisor or buyer under my own audit trail.
- **Deterministic / evidence**: export assembles only in-scope field/finding/recommendation data; every export and share writes an audit record through `10`; share access respects `10` visibility rules and is revocable.
- **Acceptance**:
  - Given an owned field, when the grower exports its summary, then a scoped artifact is produced and the export is audited.
  - Given a share link, when revoked, then subsequent access is denied; and a share that would include out-of-scope data is refused at assembly.
- **Tests**: API contract (export/share/revoke), authz (scope/revocation), failure path (out-of-scope data refused).
- **Depends on**: 13-03, 13-05, `10`.

### STORY 13-11 · M4 · L · P2 — Mobile app
- **Story**: As `GR`, I want a mobile app that shares the overview, report, and notification APIs for in-field use, so that I can check fields and reports from the tractor seat.
- **Deterministic / evidence**: the mobile shell consumes the same scoped `13` APIs (no new data authority); a read path works against cached last-known data when offline, clearly labelled with its freshness.
- **Acceptance**:
  - Given a signed-in grower on mobile, when they open the app, then the same scoped overview, inbox, and feed render as on web.
  - Given no connectivity, when the grower opens a previously loaded field, then cached data renders with an explicit "offline / as of <time>" label (never presented as live).
- **Tests**: contract (shared APIs), offline (stale-label), failure path (offline freshness label present).
- **Depends on**: 13-02, 13-03, 13-05, 13-08.

### STORY 13-12 · M4 · S · P2 — Marketplace entry point
- **Story**: As `GR`, I want a scoped link surface into the marketplace, so that I can reach buying/selling features from my portal without it owning that domain.
- **Deterministic / evidence**: render a scoped entry link to `18`, passing only the grower's resolved org/identity context; portal owns no marketplace data.
- **Acceptance**:
  - Given a grower with marketplace access, when they open the entry point, then a scoped, identity-passing link to `18` is shown.
  - Given a grower whose org has marketplace disabled, when home renders, then the entry point is hidden (not a dead/forbidden link).
- **Tests**: UI (entry render), authz (entitlement gate), failure path (disabled org hides entry).
- **Depends on**: 13-01, `18`.

### STORY 13-13 · M4 · S · P2 — Community / knowledge feed entry
- **Story**: As `GR`, I want a read-only knowledge-feed entry from the community domain, so that I can learn from shared agronomic knowledge inside the portal.
- **Deterministic / evidence**: render a read-only feed from `20` scoped to the grower's org/region; portal does not author or moderate community content.
- **Acceptance**:
  - Given community content for the grower's region, when the feed opens, then read-only items render with their source.
  - Given `20` is unavailable, when the feed opens, then a graceful "feed unavailable" state renders (no portal crash or blank).
- **Tests**: integration (read feed), failure path (`20` unavailable).
- **Depends on**: 13-01, `20`.

---

## Coverage note

These 13 stories cover all 12 capabilities in `capability-map.md` (~1 story each; recommendation tracking expands into both an M2 acknowledge slice and an M3 status-history slice). Because this is a greenfield consumption surface, the breakdown is M1/M2-weighted and almost entirely P2 — only grower identity/access (13-01) is P1, matching `release-plan.md`. There is no M5 work: the portal presents and consumes `07`/`09`/`10` read-only and owns no field, layer, or report data. The curated counts in `release-plan.md` (~64 rows) expand several of these (per-surface variants, additional mobile and export slices) into sibling stories when implemented.
