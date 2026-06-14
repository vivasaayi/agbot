# Milestone 3: Collaboration and Operations

## Goal
Support teams, recurring field work, and client delivery workflows beyond a single-user MVP.

## Why this milestone matters
A product becomes operationally sticky when it supports repeated work across clients, seasons, and team members.

## Scope
- multi-user organizations
- roles and permissions
- work orders and assignments
- field history and season context
- repeat client workflow

## Workstreams
### 1. Organization and roles
Jobs:
- add user role model and permission checks
- support advisor, operator, viewer, and admin roles
- restrict access by organization and project ownership

Acceptance:
- users only access fields, scenes, reports, and recommendations allowed by their role and organization

### 2. Work orders and task flow
Jobs:
- add `WorkOrder` domain object
- support assignment, due date, status, and linked recommendation
- expose work-order APIs and UI task lists

Acceptance:
- a recommendation can be converted into an operational task with status tracking

### 3. Field history and season context
Jobs:
- group scenes, annotations, and recommendations by field and season
- add field timeline view
- support comparison of findings over time

Acceptance:
- user can inspect field history without manually stitching reports together

### 4. Client workflow
Jobs:
- add client-facing report listing or project view
- support re-delivery and report archive
- add organization branding hooks for later white-label support

Acceptance:
- advisory team can manage multiple client deliverables and retrieve prior reports

### 5. Quality and operations
Jobs:
- add audit logs for recommendation and work-order changes
- add metrics for report generation, scene processing, and API failures
- improve error states and recovery paths

Acceptance:
- platform is supportable for pilot customers and internal troubleshooting

## Deliverables
- roles and permissions
- work orders
- field history views
- client delivery workflow
- operational observability basics

## Risks
- authorization can sprawl if added inconsistently across endpoints
- work-order flow can drift into full ERP scope if not tightly bounded
- season model can become too generic unless tied to actual use cases

## Exit criteria
- a small advisory team can use the product repeatedly across fields and clients without losing traceability
