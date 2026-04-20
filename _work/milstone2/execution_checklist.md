# GIS And Product Execution Checklist

This file is the working backlog for the agriculture GIS platform.

Tracking rules:
- Work from top to bottom unless a later item becomes a hard blocker.
- Mark an item complete only when code, tests, and basic verification are done.
- If an item is partially done, leave it unchecked and add a short note under it.
- Keep this file updated as the source of truth for progress.

Status legend:
- `[x]` completed
- `[ ]` not completed

## Phase 0: Current Baseline

These items are already done and form the starting point for the next execution cycle.

1. `[x]` Define shared GIS field geometry contracts in `shared`
2. `[x]` Add `geo_hub` field storage and field-scoped scene linking
3. `[x]` Add `geo_hub` field CRUD APIs
4. `[x]` Add scene manifests with field metadata and geospatial metadata
5. `[x]` Add GeoJSON field import in `geo_hub`
6. `[x]` Add GeoJSON field export in `geo_hub`
7. `[x]` Add field list and field-scene selection UI in `geo_viewer`
8. `[x]` Move `geo_viewer` to extent-based viewport logic
9. `[x]` Add mouse pan and zoom controls in `geo_viewer`
10. `[x]` Add cursor geospatial readout in `geo_viewer`
11. `[x]` Add field boundary overlay rendering in `geo_viewer`
12. `[x]` Add annotation contracts in `shared`
13. `[x]` Add annotation storage in `geo_hub`
14. `[x]` Add annotation create/list/update/delete APIs in `geo_hub`
15. `[x]` Add point and polygon annotation workflow in `geo_viewer`
16. `[x]` Add annotation rendering, selection, update, and delete flow in `geo_viewer`
17. `[x]` Add `shared` unit tests for GIS and annotation contracts
18. `[x]` Add `geo_hub` unit and integration tests for field, product, manifest, and annotation APIs

## Phase 1: Stabilize GIS Editing UX

These are the next items to finish so the GIS surface feels like a usable application instead of a prototype.

19. `[ ]` Split `geo_viewer` into plugins: `ui`, `network`, `map`, `annotations`
20. `[ ]` Move annotation sidebar logic out of `main.rs` into dedicated modules
21. `[ ]` Add on-map polygon vertex markers for draft polygons
22. `[ ]` Add on-map polygon vertex markers for selected saved polygons
23. `[ ]` Add vertex insertion for polygon drafts
24. `[ ]` Add vertex removal for polygon drafts
25. `[ ]` Add direct vertex drag editing for selected polygons
26. `[ ]` Add keyboard cancel for current draft annotation
27. `[ ]` Add keyboard undo for last polygon vertex
28. `[ ]` Add annotation hover/highlight behavior on the map
29. `[ ]` Add annotation labels on the map surface
30. `[ ]` Add selected annotation detail panel in `geo_viewer`
31. `[ ]` Add annotation severity color legend in `geo_viewer`
32. `[ ]` Add annotation filter controls by severity and label
33. `[ ]` Add annotation visibility toggle by geometry type
34. `[ ]` Add viewer-side tests for annotation draft helpers and map state transitions

Exit criteria for Phase 1:
- Advisor can create and edit annotations primarily from the map surface.
- Annotation workflow no longer depends on developer-style sidebar-only interaction.

## Phase 2: Tile-Based Raster Delivery

This is the main technical gap before the GIS stack is credible for large scenes.

35. `[ ]` Define tile endpoint contract in `geo_hub`
36. `[ ]` Decide initial tiling strategy: precomputed cache vs on-demand crop
37. `[ ]` Add product tile endpoint in `geo_hub` for one product kind
38. `[ ]` Add disk cache layout for tiles in `geo_hub`
39. `[ ]` Add tile cache invalidation rules for regenerated products
40. `[ ]` Add tile endpoint error-path tests in `geo_hub`
41. `[ ]` Add `geo_viewer` tile source abstraction
42. `[ ]` Replace single full-image sprite fetch with single-zoom tile rendering
43. `[ ]` Add visible tile grid calculation from camera extent
44. `[ ]` Add tile loading and unloading as the camera moves
45. `[ ]` Add tile request deduplication in `geo_viewer`
46. `[ ]` Add tile placeholder and missing-tile fallback behavior
47. `[ ]` Add tile rendering smoke checks or deterministic viewer-side tests
48. `[ ]` Verify raster alignment between tiles, field boundaries, and annotations

Exit criteria for Phase 2:
- Viewer loads a scene as map tiles, not one full raster texture.
- Pan and zoom remain usable for larger scenes.

## Phase 3: Recommendation Workflow

This is what turns the platform from a GIS viewer into an agronomy workflow tool.

49. `[ ]` Add `Recommendation` contract in `shared`
50. `[ ]` Add recommendation storage in `geo_hub`
51. `[ ]` Add recommendation CRUD APIs in `geo_hub`
52. `[ ]` Link recommendations to field, scene, and one or more annotations
53. `[ ]` Add recommendation status model: `open`, `reviewed`, `closed`
54. `[ ]` Add recommendation priority model
55. `[ ]` Add recommendation type/category model
56. `[ ]` Add integration tests for recommendation CRUD and annotation linkage
57. `[ ]` Add recommendation list and detail panel in `geo_viewer`
58. `[ ]` Add “create recommendation from selected annotation” flow in `geo_viewer`
59. `[ ]` Add recommendation update/close actions in `geo_viewer`
60. `[ ]` Add recommendation filtering by status and priority
61. `[ ]` Add recommendation summary on scene view and field view

Exit criteria for Phase 3:
- Advisor can move from finding to recommendation in one workflow.

## Phase 4: Reporting And Deliverables

This is the first directly sellable output.

62. `[ ]` Add report metadata contract in `shared`
63. `[ ]` Add report artifact storage model in backend
64. `[ ]` Decide first report generation path: HTML-to-PDF or direct PDF library
65. `[ ]` Build report data assembly pipeline from field, scene, annotations, recommendations
66. `[ ]` Add report generation endpoint in backend
67. `[ ]` Add report listing endpoint in backend
68. `[ ]` Add report download endpoint in backend
69. `[ ]` Add report integration tests for real generated artifact metadata
70. `[ ]` Add report template with field summary, scene metadata, findings, recommendations
71. `[ ]` Add map snapshot or rendered map image into the report
72. `[ ]` Add “generate report” action in `geo_viewer`
73. `[ ]` Add report history panel in `geo_viewer`
74. `[ ]` Add CSV export for annotations and recommendations
75. `[ ]` Add GeoJSON export for annotations and recommendations

Exit criteria for Phase 4:
- Advisor can produce a farmer-ready report without leaving the platform.

## Phase 5: Boundary Import Decision And Expansion

This needs a deliberate technical decision, not an accidental partial implementation.

76. `[ ]` Decide shapefile import strategy:
Notes:
- Option A: pure Rust shapefile crate
- Option B: GDAL/OGR-backed import with explicit system dependency

77. `[ ]` Document the chosen shapefile import strategy in the repo
78. `[ ]` Implement shapefile field import in `geo_hub`
79. `[ ]` Add shapefile import tests in `geo_hub`
80. `[ ]` Add import failure reporting for invalid layers and unsupported geometry
81. `[ ]` Add KML import decision note: in scope later or not
82. `[ ]` Add import UI or operator workflow entry point in `geo_viewer`

Exit criteria for Phase 5:
- Advisor can bring in field boundaries from real external GIS sources.

## Phase 6: Farm And Multi-Field Domain

This is needed for real client workflows across multiple farms and seasons.

83. `[ ]` Add `Farm` contract in `shared`
84. `[ ]` Add farm storage in backend
85. `[ ]` Link fields to farms in backend
86. `[ ]` Add farm CRUD APIs
87. `[ ]` Add field history grouped by field and season
88. `[ ]` Add field timeline or scene history view in `geo_viewer`
89. `[ ]` Add farm and field navigation hierarchy in `geo_viewer`
90. `[ ]` Add integration tests for farm-field-scene relationships

Exit criteria for Phase 6:
- Advisor can manage more than one field without flat scene browsing.

## Phase 7: Acceptance Testing And Regression Safety

This is what prevents the GIS stack from regressing as features accumulate.

91. `[ ]` Define one golden end-to-end workflow fixture
92. `[ ]` Add acceptance test: import field -> link scene -> load layer
93. `[ ]` Add acceptance test: create annotation -> update annotation -> delete annotation
94. `[ ]` Add acceptance test: create recommendation from annotation
95. `[ ]` Add acceptance test: generate and retrieve report
96. `[ ]` Add acceptance test: GeoJSON export returns expected field geometry
97. `[ ]` Add CI commands or `justfile` targets for fast GIS test runs
98. `[ ]` Document local developer commands for GIS regression runs

Exit criteria for Phase 7:
- Core advisor workflow is guarded by repeatable automated tests.

## Phase 8: Production Hardening

These items are required before treating the system as a production candidate.

99. `[ ]` Add structured request logging around `geo_hub` APIs
100. `[ ]` Add metrics for tile generation, annotation writes, report generation, and API failures
101. `[ ]` Add graceful handling for missing/invalid geospatial metadata
102. `[ ]` Add payload size limits for import endpoints
103. `[ ]` Add request timeout and background job policy for heavier processing paths
104. `[ ]` Add cache cleanup policy for tiles and generated artifacts
105. `[ ]` Add authorization model placeholder for org/user access boundaries
106. `[ ]` Add audit fields for recommendation and report changes
107. `[ ]` Document deployment assumptions for `geo_hub` and `geo_viewer`

Exit criteria for Phase 8:
- System has the minimum observability and operational controls expected for a pilot deployment.

## Working Rule For The Next Sessions

Unless a blocker appears, we should execute from item `19` onward in order.

Immediate recommended next item:
- `19. Split geo_viewer into plugins: ui, network, map, annotations`
