# Crop-Type (Species) Classification — Design & Phased Plan

Status: research synthesis + foundation (2026-07). Branch:
`crop-type-classification`. Owner domains: `05-imagery-remote-sensing`,
`07-gis-geospatial-hub`, `11-crop-intelligence`.

## Goal

Close the one substantive gap in the founding satellite-pipeline goal:
classify **crop species** (maize, winter/spring cereals, …), not just
land-cover *type* (crop vs tree vs grass). Reuse the existing
phenology-feature nearest-centroid classifier (batch 17) and the
WorldCover bootstrap-mask machinery (batch 27), swapping the reference
labels from ESA WorldCover (land cover) to **ESA WorldCereal** (crop type),
and extending the feature set to the multi-season signal crop
discrimination needs.

## What crop-type mapping actually requires (research, 2026-07)

Grounded in ESA WorldCereal — the reference global crop-mapping system.
Sources cited inline; items we could **not** verify are flagged so they
are never hard-coded from guesswork.

### WorldCereal product structure (verified)

WorldCereal v1 (2021, "v100") publishes **10 m, per-season, binary**
crop-type maps — one crop-vs-rest classifier per type, each shipping a
**class band + a confidence band** — NOT a single multi-class raster:

- `tc-annual` — temporary-crop (annual cropland) extent, binary.
- Maize — binary, per maize season.
- Winter cereals — binary.
- Spring cereals — binary (northern-hemisphere seasons).
- Active irrigation — binary, orthogonal to crop type.

(ESSD paper: https://essd.copernicus.org/articles/15/5491/2023/ ; GEE
catalog `ESA_WorldCereal_2021_MODELS_v100`.) The v2 phase (2024–2025) is a
method/system upgrade (on-demand any-area maps, more crop types via the
Presto foundation model) rather than a finalized new global product list.

**Design consequence:** our crop-type model is naturally a set of
**per-crop binary presence classifiers**, not a fixed multi-class enum
over an exhaustive taxonomy. The foundation reflects this: `CropType`
enumerates the crop classes WorldCereal actually maps, and the derivation
(later batch) is per-crop binary presence with a confidence band, mirroring
WorldCereal's own delivery.

### Reference legend (partly verified — data-driven by design)

WorldCereal's Reference Data Module (RDM) labels each sample with an
`ewoc_code` (int64, "5 numeric parts in one number", HCAT/EuroCrops-derived
hierarchy: land cover → crop group → crop type; documented example
`1111020036`). **The full code table and per-digit semantics are NOT in
the public web docs** — they live in `WorldCereal_LC_CT_legend_latest.pdf`
/ CSV in the RDM docs. Irrigation is a separate 3-digit `irrigation_status`.
(https://worldcereal.github.io/worldcereal-documentation/rdm/refdata.html)

**Design consequence:** `EwocCode` is an opaque `i64` newtype and the
`ewoc_code → CropType` mapping is a **data-driven `CropTypeLegend`** loaded
from the CSV — never a hard-coded table we cannot verify. The foundation
ships the type, the loader, and the one documented example; the real table
is an operator-supplied data file.

### Season identifiers (verified)

Per agro-ecological zone, from global crop calendars: `tc-annual`,
`tc-wintercereals`, `tc-springcereals`, `tc-maize-main`, `tc-maize-second`
(the last two optional per AEZ). These drive the temporal windows a
crop-type feature set is computed over.
(https://esa-worldcereal.org/en/products/crop-calendars)

### Features + model (verified, informs later batches)

WorldCereal drives classification from S2 optical VI **percentiles
(p10/p50/p90) + IQR**, S1 SAR (VV/VH/RVI) stats, Landsat thermal, AgERA5
meteo (GDD, ET0, precip deficit), and DEM/biome ancillary — with CatBoost
(v1) or a Presto transformer + CatBoost head (v2). Our foundation stays
with the deterministic nearest-centroid classifier we already have and
ship, but generalizes the *label space* and (later) the *feature set*
toward this recipe: per-season phenology percentiles rather than a single
season's 8 metrics.

## Foundation (this batch)

`post_processor/src/crop_type.rs` — pure domain types, no I/O:

- `CropType` — the WorldCereal-published crop classes (temporary crops,
  winter cereals, spring cereals, maize, plus `OtherCrop`), each with a
  stable numeric `code` and name.
- `WorldCerealSeason` — the five verified season identifiers with their
  string tokens.
- `EwocCode(i64)` — opaque RDM reference code newtype.
- `CropTypeLegend` — a data-driven `EwocCode → CropType` map (constructed
  from parsed CSV rows; the foundation has an empty-and-insert API and the
  documented example, NOT a fabricated table).
- `ClassLabel` trait — `code()`, `name()`, `members()` — implemented by
  **both** `CropType` and the existing `LandCoverClass`, proving the
  classifier's label space can be generalized without a rewrite.

Deliverable: the domain model + a `ClassLabel` abstraction the next batch
generalizes the nearest-centroid model over. Fully unit-tested; the ewoc
example and season tokens are pinned.

## Phased plan (after the foundation)

1. **Generalize the classifier over `ClassLabel`** — make
   `NearestCentroidModel` / `build_training_samples` generic over the label
   trait (they only use `class_code()` today); `LandCoverClass` and
   `CropType` become two label spaces of one classifier. (Refactor of the
   29 `LandCoverClass` sites in `crop_features.rs`, behavior-preserving.)
2. **Multi-season crop features** — extend phenology feature extraction to
   per-season percentiles (p10/p50/p90 + IQR of NDVI over each WorldCereal
   season window) — the WorldCereal-style feature recipe crop
   discrimination needs.
3. **WorldCereal RDM reference ingestion** — register a WorldCereal
   reference raster/vector (rasterized `ewoc_code`) as a
   `crop_type_reference` product; the `CropTypeLegend` maps codes to
   `CropType` labels for training (bootstrap masks from batch 27 apply
   unchanged).
4. **Crop-type derivation route + product** — `POST
   /api/crop-type/classify` trains per-crop presence + registers a
   `crop_type` L3 (class + confidence bands, WorldCereal-style) with
   lineage to the feature and reference products; validate against the
   reference with the batch-16 agreement/kappa engine.
5. **Browse + crop-intelligence** — `/browse` affordance, categorical
   crop-type colormap, and a crop-type finding/application surface.

## Doctrine

Same as the satellite pipeline: deterministic first (nearest-centroid over
standardized features), evidence-backed (every crop-type product traces to
its features + reference), reason-coded pixels, content-addressed identity.
Never hard-code a reference table we cannot cite — the ewoc legend is a
data file.
