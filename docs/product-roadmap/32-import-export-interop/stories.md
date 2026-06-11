# Import/Export and Interop Adapters: Detailed Stories

> Greenfield-leaning domain (M0/partial): export logic already exists scattered across `09` (`report_generator`), `geo_hub`, and `geo_viewer`, but with no CRS-preservation guarantee and no prescription/VRA export. The first stories **consolidate that into one `interop` crate** with a deterministic validate-and-reproject pipeline. The **geospatial-correctness pillar dominates every phase**: a round-trip that loses CRS or extent, or a prescription that misaligns with the field, is treated as worse than none and must be a tested rejection. The agronomic payoff is the prescription / VRA export — the bridge from a finding in `09`/`23` to an executable file in `14`.

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M3 — Explainable (the CRS-preserving interchange core)

### STORY 32-01 · M3 · M · P0 — Format-validation + CRS-reprojection pipeline
- **Story**: As `DSP`, I want every import validated and reprojected deterministically, so that bad files are caught and good files land in a known CRS.
- **Deterministic / evidence**: one pipeline parses the file, asserts a recognized CRS, reprojects to the target CRS, and emits `{source_crs, target_crs, extent, feature_count}`; malformed input and unsupported/oblique CRS are rejected with reason codes.
- **Acceptance**:
  - Given a valid file in a supported CRS, when imported, then it is reprojected to the target CRS and its extent and feature count are reported.
  - Given a file with an unrecognized or oblique CRS, when imported, then it is rejected with a CRS reason code (not imported in an unknown frame).
  - Given a structurally malformed file, when imported, then it is rejected with a parse reason code (no partial import).
- **Tests**: unit (validation + reprojection math), fixture (valid + oblique-CRS + malformed files), failure path (oblique-CRS rejected).
- **Depends on**: `07` (CRS/reprojection groundwork).

### STORY 32-02 · M3 · M · P0 — Vector import/export round-trip (Shapefile/KML/GeoJSON/GeoPackage)
- **Story**: As `AG`, I want vector layers to import and export without losing geometry or CRS, so that my data survives a trip through other tools.
- **Deterministic / evidence**: import → store → export each format; assert the exported CRS, extent, and geometry match the original within tolerance; KMZ is unzipped to KML.
- **Acceptance**:
  - Given a Shapefile or GeoJSON layer, when round-tripped (import then export), then the output preserves CRS, extent, and geometry with no coordinate drift beyond tolerance.
  - Given a GeoPackage with multiple layers, when imported, then each layer is read with its own CRS; a layer with an undeclared CRS is flagged, not assumed.
- **Tests**: round-trip (per format), geospatial (CRS/extent preserved), failure path (undeclared-CRS layer flagged).
- **Depends on**: 32-01.

### STORY 32-03 · M3 · S · P0 — Raster import/export (GeoTIFF)
- **Story**: As `DSP`, I want GeoTIFF export with asserted spatial metadata, so that a product raster opens correctly in any GIS.
- **Deterministic / evidence**: export a product raster as GeoTIFF with embedded CRS, extent, resolution, and transform; assert they match the source product.
- **Acceptance**:
  - Given a `05`/`06` product raster, when exported as GeoTIFF, then the file carries the source CRS/extent/resolution and re-opens to the same georeferencing.
  - Given a raster whose source transform is missing, when exported, then export fails with a reason code (no GeoTIFF without georeferencing).
- **Tests**: round-trip (re-open matches), unit (metadata assertion), failure path (missing transform).
- **Depends on**: 32-01, `05`/`06`.

### STORY 32-04 · M3 · M · P0 — Field-boundary import into `10`
- **Story**: As `AG`, I want to import a field boundary from a Shapefile or KML into the field model, so that I can set up a field from an existing file.
- **Deterministic / evidence**: validate geometry (closed ring, no self-intersection), reproject to the field CRS, and create/update a `10` boundary; record source and CRS.
- **Acceptance**:
  - Given a valid boundary file, when imported, then a `10` field boundary is created in the correct CRS with its area computed.
  - Given a boundary with a self-intersecting or unclosed ring, when imported, then it is rejected with a geometry reason code (no invalid boundary stored).
- **Tests**: unit (geometry validation), geospatial (reprojection + area), failure path (self-intersection rejected).
- **Depends on**: 32-01, `10`.

### STORY 32-05 · M3 · S · P1 — Report-generator export consolidation (`09`)
- **Story**: As `DSP`, I want `09`'s report/findings export routed through `interop`, so that all exports share one validated, CRS-correct path.
- **Deterministic / evidence**: `09`'s CSV/GeoJSON/KML/Shapefile findings export calls the `interop` pipeline; output validates against the same schema and CRS assertions as every other export.
- **Acceptance**: a `09` findings export produces CRS-correct GeoJSON/Shapefile through `interop`; an export with empty findings yields a valid empty file, not an error.
- **Tests**: integration (`09` → `interop`), schema validation, failure path (empty findings → valid empty export).
- **Depends on**: 32-02, `09`.

### STORY 32-06 · M3 · S · P1 — Provenance-on-import (via `30`)
- **Story**: As `OPS`, I want every import to record its source, format, and CRS, so that an imported boundary or layer stays traceable.
- **Deterministic / evidence**: on a successful import, emit a `30` lineage record `{source_filename, format, source_crs, target_crs, feature_count, operator, ts}`.
- **Acceptance**: a successful import produces a lineage record with source, format, and both CRS values; a rejected import records the rejection reason rather than a success lineage.
- **Tests**: integration (`30` emission), unit (record assembly), failure path (rejected import → rejection event, not success lineage).
- **Depends on**: 32-01, `30`.

---

## M4 — Interactive (the finding-to-action bridge and connectors)

### STORY 32-07 · M4 · M · P0 — Prescription / VRA export (Shapefile)
- **Story**: As `AG`, I want to export management zones with per-zone rates as a prescription Shapefile, so that a sprayer or planter can apply the right rate in the right place.
- **Deterministic / evidence**: assemble `{zone_polygon, rate, unit}` per zone in the field CRS; export as a prescription Shapefile with a per-zone rate attribute; assert zones tile the field without overlap.
- **Acceptance**:
  - Given `05`/`09` management zones with per-zone rates, when exported, then a prescription Shapefile is produced with one rate per zone in the field CRS.
  - Given zones that overlap or do not align with the field CRS/extent, when exported, then export is refused with an alignment reason code (no ambiguous prescription).
- **Tests**: unit (zone/rate assembly), geospatial (CRS alignment + no overlap), failure path (misaligned zones refused).
- **Depends on**: 32-02, `05`/`09`, `10`.

### STORY 32-08 · M4 · L · P0 — Prescription / VRA export (ISO-XML / ISOBUS TaskData)
- **Story**: As `GR`, I want a machine-executable ISO-XML / ISOBUS TaskData prescription, so that my ISOBUS terminal or `14` tractor executes it directly.
- **Deterministic / evidence**: encode zones+rates as ISO-XML TaskData (TASK/TZN/PDT/PGP structure) with the correct units and CRS handling; validate the output against the TaskData schema.
- **Acceptance**:
  - Given aligned zones+rates, when exported as TaskData, then a schema-valid ISO-XML TaskData set is produced that `14` can consume.
  - Given a rate with no valid unit mapping or a zone outside the field, when exported, then export fails with a reason code (no invalid TaskData emitted).
- **Tests**: unit (TaskData encoding), schema validation (ISO-XML), failure path (invalid unit/out-of-field zone).
- **Depends on**: 32-07, `14`.

### STORY 32-09 · M4 · M · P0 — John Deere Operations Center connector
- **Story**: As `DSP`, I want to exchange boundaries and prescriptions with John Deere Operations Center, so that growers using JD equipment can use AGBot output.
- **Deterministic / evidence**: a connector behind a mockable external boundary pushes a prescription and pulls boundaries; the platform mapping (CRS, units) is validated on both directions; runs simulation-first against a mock.
- **Acceptance**:
  - Given a valid prescription and a mock JD endpoint, when pushed, then the connector maps and uploads it and reports success with the remote ID.
  - Given the endpoint failing or returning an error, when pushed, then the connector retries with backoff and surfaces a clear failure (no silent loss, no partial push claimed as success).
- **Tests**: integration (mock endpoint), unit (mapping/units), failure path (endpoint error → retry then surfaced failure).
- **Depends on**: 32-07, 32-08.

### STORY 32-10 · M4 · S · P1 — Climate FieldView connector
- **Story**: As `DSP`, I want to exchange boundaries and prescriptions with Climate FieldView, so that FieldView growers can use AGBot output.
- **Deterministic / evidence**: a connector behind a mockable boundary exchanges boundaries/prescriptions with FieldView's mapping; CRS/units validated; simulation-first.
- **Acceptance**: a prescription pushes to a mock FieldView endpoint with correct mapping; an unsupported field/CRS mapping is refused with a reason code, not coerced.
- **Tests**: integration (mock endpoint), unit (mapping), failure path (unsupported mapping refused).
- **Depends on**: 32-07, 32-09.

### STORY 32-11 · M4 · S · P2 — Trimble connector
- **Story**: As `DSP`, I want to exchange boundaries and prescriptions with Trimble, so that Trimble-equipped operations can use AGBot output.
- **Deterministic / evidence**: a connector behind a mockable boundary exchanges boundaries/prescriptions with Trimble's mapping; CRS/units validated; simulation-first.
- **Acceptance**: a boundary imports from a mock Trimble endpoint into `10` with correct CRS; an endpoint timeout is retried and then surfaced (no half-imported boundary).
- **Tests**: integration (mock endpoint), geospatial (CRS), failure path (timeout → retry then surfaced).
- **Depends on**: 32-04, 32-09.

### STORY 32-12 · M4 · M · P1 — Bulk import/export with per-row report
- **Story**: As `OPS`, I want to import or export many layers at once and get a per-row outcome report, so that I can onboard or migrate data at scale.
- **Deterministic / evidence**: process a batch through the same validate/reproject pipeline; produce a per-row report `{row, status, reason_code}`; one bad row does not fail the batch.
- **Acceptance**:
  - Given a batch of boundary files, when bulk-imported, then each row reports success or a reason-coded failure and the valid rows are imported.
  - Given a batch where some rows are malformed, when imported, then the malformed rows are reported as failed and the rest still import (partial success is explicit, not silent).
- **Tests**: integration (mixed-validity batch), unit (per-row reporting), failure path (some rows fail, batch continues).
- **Depends on**: 32-01, 32-04.

---

## M5 — Autonomous-Assist (migration at scale)

### STORY 32-13 · M5 · M · P2 — Platform migration round-trip
- **Story**: As `PA`, I want to migrate an entire org's boundaries and prescriptions from an external platform and verify nothing was lost, so that a customer can switch with confidence.
- **Deterministic / evidence**: pull all boundaries/prescriptions via a connector, import through the validated pipeline, and produce a reconciliation report comparing source vs imported counts, areas, and CRS.
- **Acceptance**:
  - Given a mock source platform, when migrated, then every boundary/prescription is imported and the reconciliation report shows matching counts and areas within tolerance.
  - Given a source item that fails validation, when migrated, then it is listed as unmigrated with a reason and the reconciliation report flags the discrepancy (no silently dropped field).
- **Tests**: integration (mock platform migration), unit (reconciliation), failure path (invalid item flagged in reconciliation).
- **Depends on**: 32-09, 32-12.

### STORY 32-14 · M5 · S · P2 — Round-trip fidelity certification suite
- **Story**: As `PA`, I want a standing certification suite that proves every supported format round-trips with CRS fidelity, so that interop fidelity is continuously guaranteed.
- **Deterministic / evidence**: a suite runs import→export→re-import across all supported formats and asserts CRS/extent/geometry equality within tolerance; failures name the format and the divergence.
- **Acceptance**: the suite passes for all supported formats; an introduced CRS-dropping regression in any format fails the suite and names the offending format (fidelity is enforced, not assumed).
- **Tests**: certification suite (all formats), failure path (injected CRS-drop regression fails and is named).
- **Depends on**: 32-02, 32-03, 32-07, 32-08.

---

## Coverage note

These 14 stories cover all 12 capabilities in `capability-map.md` (the three platform connectors expand into 32-09/32-10/32-11). The breakdown carries a heavy M3 interchange core — the validate/reproject pipeline, vector/raster round-trip, and boundary import — reflecting that **geospatial correctness leads every phase** in `release-plan.md`: a round-trip that loses CRS, or a misaligned prescription, is the central tested failure path. The M4 prescription / VRA export (32-07/32-08) is the agronomic payoff, the bridge from finding to action consumed by `14`. The curated counts in `release-plan.md` (~69 rows) expand several of these (per-format import/export variants, additional connector operations, migration slices) into sibling stories when implemented.
