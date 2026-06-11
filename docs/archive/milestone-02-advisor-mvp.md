# Milestone 2: Advisor MVP

## Goal
Ship the first sellable advisor workflow: ingest, inspect, annotate, recommend, and report.

## Why this milestone matters
This is the first milestone that creates a directly sellable product outcome.

## Scope
- layer inspection workflow
- annotations
- recommendations
- PDF report export
- basic shareable output

## Workflow target
1. select a field
2. load a recent scene
3. inspect NDVI or thermal layer
4. mark one or more issue areas
5. create a recommendation
6. export and share a report

## Workstreams
### 1. Annotation model and APIs
Jobs:
- add `Annotation` contract to `shared`
- store point and polygon annotations in backend
- add annotation APIs in `geo_hub`
- include category, severity, note, creator, timestamps, and linked scene and field

Acceptance:
- annotations can be created, listed, and updated through API
- annotations retain authorship and timestamps

### 2. Viewer annotation workflow
Jobs:
- add point and polygon drawing support in `geo_viewer`
- allow user to tag issue type and severity
- render saved annotations on top of scene and boundary
- support annotation list and selection

Acceptance:
- advisor can create, view, edit, and delete annotations in the UI

### 3. Recommendation workflow
Jobs:
- add `Recommendation` contract to `shared`
- create backend storage and APIs
- link recommendations to field, scene, and annotations
- support status and priority

Acceptance:
- recommendation can be created from annotation context and retrieved later

### 4. Report export
Jobs:
- design report template
- generate PDF containing field metadata, scene metadata, screenshots or map views, annotations, and recommendations
- export CSV and GeoJSON for findings

Acceptance:
- report can be generated from a real field workflow
- output is suitable for a farmer or client handoff

### 5. Basic sharing
Jobs:
- generate shareable artifact or share link for report
- add simple viewer-safe delivery option for clients
- ensure access is bounded by report visibility rules

Acceptance:
- advisor can deliver report without requiring the client to navigate developer-oriented UI

### 6. Acceptance tests
Jobs:
- add integration tests for annotation and recommendation APIs
- add one acceptance scenario:
  - field exists
  - scene exists
  - annotation created
  - recommendation created
  - report exported

Acceptance:
- the advisor MVP path is reproducible and testable

## Deliverables
- annotation APIs and UI
- recommendation APIs and UI
- report export capability
- first sellable end-to-end workflow

## Risks
- report generation can sprawl if branding and layout are overdesigned too early
- annotation UX can become Bevy-heavy if not isolated into smaller plugins
- backend model complexity can outpace current UI structure

## Exit criteria
- advisor can complete ingest-to-report flow in one platform
- output is clear enough to show a pilot customer
