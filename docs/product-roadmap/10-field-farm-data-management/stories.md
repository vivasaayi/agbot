# Field, Farm, and Data Management: Detailed Stories

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. This is the Phase 0 product spine: identity and tenant isolation are non-negotiable P0, and every entity belongs to an organization with a traceable history. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence** (or **Safety** / **Operability** where it fits): what must be enforced and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M1 — Foundation (the tenant-safe spine)

### STORY 10-01 · M1 · M · P0 — Organization and user model
- **Story**: As `PA`, I want organizations and users to be first-class persisted records with membership, so that every downstream entity has an owner and a tenant.
- **Deterministic / evidence**: persist `{org_id, name, created_at}` and `{user_id, email, org_id, status}`; a membership row links a user to an org with a join timestamp; email is unique within the system.
- **Acceptance**:
  - Given a platform admin, when an org and a user are created, then both persist with stable IDs and the membership links the user to the org.
  - Given a duplicate email, when a second user is created with it, then creation is rejected with a conflict error and no row is written.
- **Tests**: unit (uniqueness + linkage), API contract (create/list/get), failure path (duplicate email → 4xx).
- **Depends on**: `shared` config/runtime, new control-plane crate.

### STORY 10-02 · M1 · M · P0 — Roles and access control
- **Story**: As `PA`, I want each membership to carry a role (admin/advisor/operator/viewer), so that the system can authorize actions deterministically.
- **Deterministic / evidence**: role enum persisted on membership; a static permission matrix maps `(role, action)` → allow/deny; every mutation checks the matrix before writing; denials are logged with the attempted action.
- **Acceptance**:
  - Given a viewer membership, when the user attempts a write action, then it is denied with an authorization error and audited.
  - Given an admin membership, when the user performs the same action, then it succeeds.
  - Given an unknown role value, when authorization runs, then it fails closed (deny), never defaults to allow.
- **Tests**: unit (permission matrix incl. fail-closed), API contract (per-role action checks), failure path (viewer write → 403).
- **Depends on**: 10-01.

### STORY 10-03 · M1 · M · P0 — Tenant isolation on every path
- **Story**: As `PA`, I want every read and write scoped to the caller's organization, so that no tenant can ever see or touch another tenant's data.
- **Deterministic / evidence**: every query is filtered by `org_id` derived from the authenticated principal, not from request input; cross-org access returns not-found (not forbidden) to avoid existence leaks; isolation is enforced at the data-access layer, not per-endpoint.
- **Acceptance**:
  - Given a user in org A, when they request an entity owned by org B, then the response is `404` as if it did not exist.
  - Given a write that names an `org_id` different from the principal's org, when it executes, then it is rejected and nothing is written.
- **Tests**: unit (scope injection at access layer), API contract (cross-tenant read/write), failure path (cross-org access → 404, audited).
- **Depends on**: 10-01, 10-02.

### STORY 10-04 · M1 · S · P0 — Tenant boundary audit trail
- **Story**: As `PA`, I want every cross-org access attempt and every privileged action recorded, so that isolation is provable, not assumed.
- **Deterministic / evidence**: append-only audit record `{actor_user_id, org_id, action, target_ref, decision, at}`; records are immutable and queryable by org and time range.
- **Acceptance**:
  - Given any allow or deny decision, when it occurs, then an immutable audit record is written with actor, decision, and target.
  - Given an audit record, when an update is attempted, then it is rejected (append-only).
- **Tests**: unit (append-only enforcement), API contract (audit query by org/time), failure path (update attempt → rejected).
- **Depends on**: 10-02, 10-03.

### STORY 10-05 · M1 · M · P0 — Farm and field entities
- **Story**: As `GR`, I want farms and fields persisted as owned records under my organization, so that scenes, findings, and reports have a real field to attach to.
- **Deterministic / evidence**: replace the loose `shared` `FarmRecord`/`FieldRecord` structs with persisted, org-owned entities `{farm_id, org_id, name}` and `{field_id, farm_id, org_id, name, area_ha?}`; a field always resolves to a farm and an org.
- **Acceptance**:
  - Given an org, when a farm and a field are created, then both persist with org/farm linkage and are listable under that org only.
  - Given a field created with a `farm_id` from another org, when it executes, then it is rejected (tenant boundary).
- **Tests**: unit (linkage), API contract (CRUD + list under org), failure path (cross-org farm_id → rejected).
- **Depends on**: 10-03, `shared/src/schemas.rs` (FarmRecord/FieldRecord).

### STORY 10-06 · M1 · M · P0 — Field boundary management
- **Story**: As `AG`, I want a field's boundary stored and validated as geometry, so that coverage, area, and overlays are computed against a real shape.
- **Deterministic / evidence**: persist `FieldBoundary` as a polygon with declared CRS and computed area; validate geometry (closed ring, no self-intersection) and assert extent on store; reject invalid or empty geometry.
- **Acceptance**:
  - Given a valid polygon with a CRS, when it is stored, then area and extent are computed and the boundary round-trips with the same CRS.
  - Given a self-intersecting or unclosed polygon, when it is stored, then it is rejected with a geometry-validation error and nothing is persisted.
- **Tests**: unit (geometry validation + area), geospatial round-trip (CRS/extent preserved), failure path (invalid polygon → rejected).
- **Depends on**: 10-05, `shared` (FieldBoundary, GeoBounds, GeoPoint).

### STORY 10-07 · M1 · M · P0 — GeoJSON boundary import
- **Story**: As `AG`, I want to import a field boundary from a GeoJSON file with its CRS asserted, so that I can onboard a field from existing GIS data (MVP requirement).
- **Deterministic / evidence**: parse GeoJSON Feature/FeatureCollection, take the polygon, assert/normalize CRS (default WGS84 per spec), validate geometry via 10-06, and re-emit as canonical GeoJSON for round-trip; shapefile/KML deferred.
- **Acceptance**:
  - Given a valid GeoJSON polygon, when imported, then a `FieldBoundary` is stored and re-exporting it yields equivalent geometry in the correct CRS.
  - Given a GeoJSON with a non-polygon geometry or a missing/unsupported CRS, when imported, then it is rejected with a clear reason and no boundary is created.
- **Tests**: unit (parse + CRS normalization), geospatial round-trip (import→export equivalence), failure path (non-polygon / bad CRS → rejected).
- **Depends on**: 10-06; supersedes shapefile-only storage in `geo_hub`.

### STORY 10-08 · M1 · S · P1 — Entity listing, pagination, and lifecycle state
- **Story**: As `DSP`, I want farms, fields, and boundaries listable with pagination and a lifecycle state, so that I can manage many fields without loading everything at once.
- **Deterministic / evidence**: each entity carries `{status: active|archived, created_at, updated_at}`; list endpoints paginate and filter by status; archived entities are excluded from default lists.
- **Acceptance**:
  - Given many fields, when listed with a page size, then results paginate deterministically and respect org scope.
  - Given an archived field, when the default list runs, then it is excluded unless explicitly requested.
- **Tests**: API contract (pagination + status filter), fixture (seeded entities), failure path (page beyond range → empty page, not error).
- **Depends on**: 10-05, 10-06.

---

## M2 — Captured / Observable (field context over time)

### STORY 10-09 · M2 · M · P0 — Season and crop-plan history
- **Story**: As `AG`, I want seasons and crop plans linked to a field, so that flights and findings can be compared within and across seasons.
- **Deterministic / evidence**: persist `{season_id, field_id, org_id, start, end, label}` and `{crop_plan_id, season_id, crop, planting_date?}`; seasons for a field may not overlap in time; a field's history is queryable chronologically.
- **Acceptance**:
  - Given a field, when a season and crop plan are created, then both link to the field and appear in its chronological history.
  - Given a season whose date range overlaps an existing season for the same field, when it is created, then it is rejected with an overlap error.
- **Tests**: unit (overlap detection), API contract (history query), failure path (overlapping season → rejected).
- **Depends on**: 10-05.

### STORY 10-10 · M2 · S · P1 — Season-scoped context resolution
- **Story**: As `AG`, I want every field query to optionally resolve "current/active season," so that the advisor workflow defaults to the right temporal context.
- **Deterministic / evidence**: given a date, deterministically resolve the active season for a field (the season whose range contains it); return an explicit "no active season" when none matches, never a guess.
- **Acceptance**:
  - Given a field with seasons, when the active season is requested for a date in range, then exactly that season is returned.
  - Given a date outside all seasons, when resolution runs, then it returns "no active season," not the nearest one.
- **Tests**: unit (range resolution), failure path (no matching season).
- **Depends on**: 10-09.

### STORY 10-11 · M2 · M · P0 — Scene and layer registry
- **Story**: As `DSP`, I want scenes and their layers owned by a field and season with a product catalog, so that `07`/`08` can resolve imagery and products by field context.
- **Deterministic / evidence**: persist `{scene_id, field_id, season_id, org_id, captured_at, source}` and `{layer_id, scene_id, product_type, crs, extent, resolution, uri}`; a scene always resolves to a field and season; layers assert CRS/extent/resolution on registration.
- **Acceptance**:
  - Given a field and season, when a scene with layers is registered, then each layer persists its product type and geospatial metadata and is listable by field/season.
  - Given a layer missing CRS or extent, when registered, then it is rejected with a metadata error.
- **Tests**: unit (linkage + metadata assertion), API contract (register/list by field/season), failure path (layer without CRS → rejected).
- **Depends on**: 10-09; consumed by `07` (GIS hub), `08` (viewer), `09` (advisor).

### STORY 10-12 · M2 · S · P1 — Scene freshness and coverage
- **Story**: As `AG`, I want each scene to expose capture freshness and field coverage, so that I know whether a report is built on current, complete data.
- **Deterministic / evidence**: compute coverage as the fraction of the field boundary intersected by the scene's layer extents; record `captured_at` age; mark a scene stale beyond a configurable threshold.
- **Acceptance**:
  - Given a scene and a field boundary, when coverage is computed, then it returns a valid fraction in `[0,1]` in the boundary's CRS.
  - Given a scene whose layers do not intersect the field, when coverage runs, then it returns `0` and flags "no coverage," not an error.
- **Tests**: unit (coverage fraction + freshness), geospatial (intersection in correct CRS), failure path (non-intersecting scene → 0/no-coverage).
- **Depends on**: 10-06, 10-11.

---

## M3 — Explainable (audited collaboration records)

### STORY 10-13 · M3 · M · P0 — Annotation persistence and audit
- **Story**: As `AG`, I want annotations persisted with author and full change history, so that what I marked on a field is accountable and re-openable.
- **Deterministic / evidence**: persist `AnnotationRecord` `{annotation_id, field_id, scene_id?, org_id, author_user_id, geometry, created_at}`; every edit appends an immutable change record `{actor, before/after, at}`; annotations are org-scoped.
- **Acceptance**:
  - Given an agronomist, when an annotation is created and later edited, then both the author and the edit are recorded in an append-only history.
  - Given a request to hard-delete history, when it executes, then it is rejected; only a soft "retracted" state is allowed.
- **Tests**: unit (append-only history), API contract (create/edit/list with author), failure path (history delete → rejected).
- **Depends on**: 10-04 (audit), 10-11; consumed by `08`/`09`.

### STORY 10-14 · M3 · M · P0 — Recommendation persistence and status lifecycle
- **Story**: As `AG`, I want recommendations persisted with an audited status lifecycle, so that a grower's next steps are tracked from open to done.
- **Deterministic / evidence**: persist `RecommendationRecord` `{rec_id, field_id, org_id, author_user_id, priority, action_category, status, evidence_refs[]}`; status transitions `open→reviewed→completed|dismissed` are validated and each transition is audited; a recommendation must cite at least one evidence ref.
- **Acceptance**:
  - Given a recommendation, when its status moves open→reviewed→completed, then each transition is audited with actor and timestamp.
  - Given an invalid transition (e.g. completed→open) or a recommendation with no evidence ref, when attempted, then it is rejected.
- **Tests**: unit (transition validation + evidence requirement), API contract (create/transition), failure path (illegal transition → rejected).
- **Depends on**: 10-04, 10-13; produced by `09` (advisor).

### STORY 10-15 · M3 · S · P1 — Report and deliverable records
- **Story**: As `DSP`, I want generated reports persisted as records with their source linkage, so that every delivered PDF is traceable to the field, season, and findings it came from.
- **Deterministic / evidence**: persist `ReportRecord` `{report_id, field_id, season_id, org_id, generated_by, source_refs[], artifact_uri, visibility}`; a report always links its source scene/findings; visibility is one of `org|shared`.
- **Acceptance**:
  - Given a completed analysis, when a report record is created, then it links field/season/source findings and stores the artifact reference.
  - Given a report record missing its source linkage, when created, then it is rejected (no orphan deliverables).
- **Tests**: unit (linkage), API contract (create/list by field/season), failure path (missing source → rejected).
- **Depends on**: 10-11, 10-14; produced by `09`.

### STORY 10-16 · M3 · S · P2 — Provenance and reproducibility ledger
- **Story**: As `PA`, I want each derived record (scene→annotation→recommendation→report) to retain its lineage, so that any deliverable can be traced end-to-end.
- **Deterministic / evidence**: maintain a lineage graph linking source and derived entity IDs; a report's lineage resolves to its scenes, layers, annotations, and recommendations deterministically.
- **Acceptance**:
  - Given a report, when its lineage is queried, then it resolves the full chain of source entities.
  - Given a broken link (a referenced source was retracted), when lineage is resolved, then the gap is reported explicitly, not silently dropped.
- **Tests**: unit (graph traversal), API contract (lineage query), failure path (retracted source → explicit gap).
- **Depends on**: 10-13, 10-14, 10-15.

---

## M4 — Interactive (work orders and shareable delivery)

### STORY 10-17 · M4 · M · P0 — Work order from a recommendation
- **Story**: As `GR`, I want to turn a recommendation into a work order with an assignee and a task lifecycle, so that field action is tracked to completion.
- **Deterministic / evidence**: persist `WorkOrder` `{wo_id, field_id, org_id, source_rec_id, assignee_user_id?, status, due?}`; lifecycle `created→assigned→in_progress→done|cancelled`, each transition audited; a work order must originate from a recommendation.
- **Acceptance**:
  - Given an open recommendation, when a work order is created from it, then it links the recommendation and starts in `created`, audited.
  - Given a work order created without a source recommendation, when attempted, then it is rejected.
- **Tests**: unit (lifecycle transitions), API contract (create/assign/transition), failure path (no source rec → rejected).
- **Depends on**: 10-14.

### STORY 10-18 · M4 · S · P1 — Work order assignment and operator handoff
- **Story**: As `OPS`, I want work orders assigned to me to appear in a scoped, filterable queue, so that I can see what I must execute without scanning other tenants or fields.
- **Deterministic / evidence**: queue lists work orders filtered by assignee and org with status filters; reassignment is audited; an operator only sees work orders within their org.
- **Acceptance**:
  - Given assigned work orders, when an operator opens their queue, then only org-scoped, assigned items appear, filterable by status.
  - Given a reassignment to a user outside the org, when attempted, then it is rejected and audited.
- **Tests**: API contract (scoped queue + filters), unit (reassignment authz), failure path (cross-org assignee → rejected).
- **Depends on**: 10-03, 10-17.

### STORY 10-19 · M4 · S · P0 — Shareable report delivery with bounded visibility
- **Story**: As `AG`, I want to share a report via a link with bounded, revocable visibility, so that a grower can view a deliverable without a system account.
- **Deterministic / evidence**: generate a share artifact/link bound to a report record; access respects report `visibility` and an expiry; revocation invalidates the link immediately; share events are audited.
- **Acceptance**:
  - Given a `shared`-visibility report, when a link is generated, then an out-of-system viewer can open it until expiry or revocation.
  - Given a revoked or expired link, when accessed, then access is denied; an `org`-only report never produces a public link.
- **Tests**: API contract (share/revoke/expire), authz (org-only report not shareable), failure path (revoked link → denied).
- **Depends on**: 10-15; consumed by `09` (report delivery), `08`.

### STORY 10-20 · M4 · S · P2 — Export field record bundle (CSV + GeoJSON)
- **Story**: As `DSP`, I want a field's records exported as CSV and GeoJSON, so that a client can take their data into other tools.
- **Deterministic / evidence**: export boundaries/annotations/zones as GeoJSON (correct CRS + properties) and recommendations/work orders as CSV; both validate against a schema; export is org-scoped.
- **Acceptance**:
  - Given a field with records, when exported, then GeoJSON geometry carries the correct CRS and CSV rows match the record set.
  - Given a field with no records, when exported, then a valid empty export is produced, not an error.
- **Tests**: geospatial round-trip, schema validation, failure path (empty field → valid empty export).
- **Depends on**: 10-06, 10-14, 10-17.

---

## M5 — Autonomous-Assist (gated, advisory)

### STORY 10-21 · M5 · M · P2 — Suggested season/crop-plan rollover
- **Story**: As `AG`, I want a suggested next-season setup proposed from a field's history, so that I can start a new season quickly while staying in control.
- **Deterministic / evidence**: suggestion derived only from persisted season/crop-plan history; presented as a proposal requiring explicit approval before any record is written; the source history is cited.
- **Acceptance**:
  - Given a field with prior seasons, when a rollover is suggested, then it proposes (never auto-creates) a season/crop plan and cites the history it used.
  - Given no prior history, when a rollover is requested, then it returns "no basis to suggest," not a fabricated plan.
- **Tests**: unit (suggestion from history), gating test (no auto-write without approval), failure path (no history → no suggestion).
- **Depends on**: 10-09, 10-10.

### STORY 10-22 · M5 · S · P2 — Anomalous-access advisory
- **Story**: As `PA`, I want an advisory flag when access patterns look anomalous (e.g. cross-org probing, bulk export), so that I can investigate without auto-locking tenants.
- **Deterministic / evidence**: anomaly signals computed from the deterministic audit trail (denied cross-org attempts, export volume); surfaced as an advisory with the evidence records, never an automatic block; approval-gated for any enforcement.
- **Acceptance**:
  - Given a spike of denied cross-org attempts, when the advisory runs, then it flags the actor and cites the audit records.
  - Given normal access, when the advisory runs, then no flag is raised (no false positives on baseline traffic).
- **Tests**: unit (signal thresholds), failure path (baseline → no flag), gating test (advisory only, no auto-block).
- **Depends on**: 10-04.

---

## Coverage note

These ~22 stories cover the 12 capabilities in `capability-map.md`, ordered by phase and weighted toward M1 because this domain is the Phase 0 product spine. Identity, roles, tenant isolation, and the audit trail (10-01..10-04) are front-loaded P0 per `release-plan.md`, which curates ≈78 feature rows (M1 28 / M2 16 / M3 16 / M4 14 / M5 4). Several stories here expand into sibling rows when implemented — per-entity CRUD variants, additional boundary formats (shapefile/KML), and per-record export schemas. Tenant isolation and audit are enforced as cross-cutting acceptance criteria on every story, not just their dedicated ones. Domains `07`/`08`/`09` resolve all field context, ownership, and traceability through this spine.
