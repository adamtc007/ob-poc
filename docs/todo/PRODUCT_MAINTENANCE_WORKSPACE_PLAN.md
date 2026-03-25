# ProductMaintenance Workspace Implementation Plan

## Purpose

Introduce a design-time workspace that owns the catalog taxonomy:

`Product -> Service -> Servicing Resource -> Resource Dictionary`

This separates catalog maintenance from runtime activation.

## Core Model

- `ProductMaintenance` owns the governed taxonomy and reference catalog.
- `OnBoarding` consumes that taxonomy to derive activation requirements.
- `CBU` owns instantiated resource and lifecycle instances.

Canonical runtime chain:

`CBU onboarding -> Product subscription -> Service discovery -> Required resource discovery -> CBU resource activation`

## New Workspace

- `Workspace`: `ProductMaintenance`
- `Primary constellation family`: `product_service_taxonomy`
- `Primary constellation map`: `product.service.taxonomy`

## Constellation Shape

The design-time DAG should be:

`product -> service -> service_resource -> resource_dictionary`

Supporting rules:

- a product maps to `1..n` services
- a service maps to `0..n` servicing resources
- a servicing resource may declare `0..n` required attributes from the dictionary

## DSL Node Verbs

Use existing verbs first. Do not invent a new write DSL in this slice.

- `product`: `product.read`, `product.list`
- `service`: `service.read`, `service.list`, `service.list-by-product`
- `service_resource`: `service-resource.read`, `service-resource.list`, `service-resource.list-by-service`, `service-resource.list-attributes`
- `resource_dictionary`: `attribute.read`, `attribute.list`

Agent/discovery surfaces must keep these separate from runtime activation verbs:

- taxonomy discovery:
  - `product.read`, `product.list`
  - `service.read`, `service.list`, `service.list-by-product`
  - `service-resource.read`, `service-resource.list`, `service-resource.list-by-service`, `service-resource.list-attributes`
  - `attribute.read`, `attribute.list`
- runtime activation:
  - `service-resource.provision`
  - `service-resource.set-attr`
  - `service-resource.activate`
  - `service-resource.suspend`
  - `service-resource.decommission`
  - `service-resource.validate-attrs`

## Runtime Boundary

These remain runtime activation verbs, not ProductMaintenance taxonomy verbs:

- `service-resource.provision`
- `service-resource.set-attr`
- `service-resource.activate`
- `service-resource.suspend`
- `service-resource.decommission`
- `service-resource.validate-attrs`

They belong on the activation side under `OnBoarding` / `CBU`.

## Implementation Steps

1. Add `ProductMaintenance` to workspace taxonomies and validation allowlists.
2. Map `product`, `service`, and `service-resource` domains into `ProductMaintenance`.
3. Add `attribute` to `ProductMaintenance` to support resource dictionary lookups.
4. Add a new constellation family: `product_service_taxonomy`.
5. Add a new constellation map: `product.service.taxonomy`.
6. Update session/workspace planning docs so `ProductMaintenance` is a first-class top-level workspace.
7. Re-run taxonomy and workspace-affinity artifacts.
8. Reclassify the remaining `service-resource.*` unresolved plugin verbs as runtime activation binding gaps, not catalog gaps.

## Acceptance

- `ProductMaintenance` is recognized as a valid workspace by SemOS footprint tooling.
- `product/service/service-resource` taxonomy verbs appear in the new workspace family.
- The new constellation map resolves the catalog DAG cleanly.
- Remaining unresolved `service-resource` instance verbs are explicitly framed as runtime activation gaps.
- `cargo check` passes.
