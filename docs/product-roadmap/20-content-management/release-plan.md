# Content Management System: Release Plan

## Shipment Strategy

This is a greenfield (M0 named) domain, so the plan is weighted to the M1 foundation and M2 captured phases: first establish the content model and editorial workflow under `10` access control, then make content discoverable (M2 knowledge base, search, taxonomy), then explainable/embeddable (M3 portal embedding and engagement analytics), then interactive community contribution and localization (M4). Because this domain is the most decoupled on the roadmap, it can run as a near-standalone workstream; priority is still mostly P2 (post-MVP) with only the foundational content-model slice P1, since the core drone platform and advisor MVP come first.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 18 |
| M2 captured | 16 |
| M3 explainable | 12 |
| M4 interactive | 12 |
| M5 autonomous-assist | 2 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P1 | 6 |
| P2 | 44 |
| P3 | 10 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 6 |
| M | 30 |
| S | 24 |

## First P0/P1 Vertical Slices

| Phase | Size | Priority | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- | --- |
| M1 foundation | S | P1 | Content model (articles/guides/posts) | operability | identity |
| M1 foundation | M | P2 | Authoring and editorial workflow | operability | workflow |
| M1 foundation | S | P2 | Access control (via `10` roles) | explainability and trust | identity |
| M2 captured | M | P2 | Knowledge base and search | operability | capture |
| M2 captured | S | P2 | Categorization and tagging | data quality | capture |
| M3 explainable | S | P2 | Portal embedding (into `13`) | operability | operations |
| M4 interactive | M | P2 | Community contributions and moderation | operability | operations |

## Execution Rules

- Sequenced after the core drone platform (`01`-`12`) and gated by the advisor MVP, but as the most decoupled domain it can proceed as a near-standalone workstream once `10` roles exist.
- No content is published without passing the editorial workflow; community contributions must clear moderation before going live.
- Authoring, editorial, and moderation permissions must resolve through the `10` role model — no unscoped write path.
- Any AI assist (tag suggestion, summarization) is advisory only and never auto-publishes; an editor approves before publish.
