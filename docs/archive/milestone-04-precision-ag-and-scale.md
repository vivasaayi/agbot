# Milestone 4: Precision Ag Expansion and Scale

## Goal
Expand from report-centric workflow into operational agronomy outputs and scalable delivery.

## Why this milestone matters
This milestone moves the platform from useful reporting to stronger agronomic and operational value.

## Scope
- time-series analysis
- prescriptions and management zones
- anomaly-detection support
- large-raster performance
- alerts and enterprise hardening

## Workstreams
### 1. Time-series and comparison
Jobs:
- add scene comparison by field and season
- support difference views and trend summaries
- expose time-series APIs and viewer controls

Acceptance:
- advisor can compare field condition across multiple dates on the same field

### 2. Prescription and management zones
Jobs:
- support derived management zones from annotations or analytics
- export machine-friendly zone formats where practical
- store prescription metadata and version history

Acceptance:
- platform can output structured agronomic action layers, not only static reports

### 3. Anomaly-detection support
Jobs:
- extend `imagery_processor` with anomaly-support outputs and deterministic tests
- expose anomaly candidate layers to the viewer
- keep analyst review in the loop; do not ship opaque auto-diagnosis

Acceptance:
- anomalies are surfaced as decision-support candidates and remain explainable

### 4. Large-raster and tile performance
Jobs:
- move viewer toward proper map viewport and tile strategy
- add caching, background jobs, and better product serving behavior in `geo_hub`
- improve memory stability and load performance for large scenes

Acceptance:
- platform remains responsive on production-scale imagery instead of only small demo scenes

### 5. Hardening
Jobs:
- add metrics, tracing, and failure dashboards
- add release quality gates and acceptance tests
- improve data migration and backup posture

Acceptance:
- product is operationally supportable for paid usage

## Deliverables
- time-series field review
- prescription-ready outputs
- anomaly-support layers
- performance and operational hardening

## Risks
- prescription workflows require careful format and machine compatibility decisions
- anomaly features can create liability if marketed as diagnosis instead of support
- scale work can consume large effort without clear usage metrics

## Exit criteria
- product supports repeated, scalable field analysis and operational outputs beyond static reporting
