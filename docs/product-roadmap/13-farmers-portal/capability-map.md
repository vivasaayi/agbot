# Farmers Portal: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (operability, explainability and trust, agronomic value, data quality, geospatial correctness) and the workstreams in `release-plan.md`. Because this is a greenfield domain (M0 named), every capability's current source status is "missing (greenfield)", and the Primary First Slice describes the M1 foundation step. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Farmers Portal Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Grower account and org/role access (via `10`) | missing (greenfield) | 7 | Sign-in scoped to org/role resolved through `10` |
| Grower dashboard (home) | missing (greenfield) | 7 | Render a per-grower home from `10` farms/fields |
| Field and farm overview | missing (greenfield) | 8 | Aggregate latest scene/finding per field read-only |
| Report inbox (consumes `09` reports) | missing (greenfield) | 8 | List and open advisor reports owned by the grower |
| Recommendation tracking | missing (greenfield) | 7 | Acknowledge a recommendation with audited status |
| Notifications and alert feed | missing (greenfield) | 6 | Notify on a new report for an owned field |
| Mobile app | missing (greenfield) | 9 | Mobile shell sharing the overview/report APIs |
| Field map and layer view (via `07`/`08`) | missing (greenfield) | 6 | Read-only field map with one GIS layer overlay |
| Marketplace entry point (-> `18`) | missing (greenfield) | 4 | Scoped link surface to the marketplace domain |
| Community / knowledge feed (-> `20`) | missing (greenfield) | 5 | Read-only knowledge feed entry from `20` |
| Data export and sharing | missing (greenfield) | 6 | Export a grower-scoped field summary, audited |
| Saved views and preferences | missing (greenfield) | 4 | Persist a grower's default farm/field view |
