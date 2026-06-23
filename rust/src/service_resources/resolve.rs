//! Pure catalogue-backed service-resource resolution.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use super::discovery::extract_srdef_parameters;
use super::srdef_loader::{
    load_srdefs_from_config, LoadedSrdef, LoadedSrdefAttribute, SrdefRegistry,
};

/// Side-effect-free output for `Resolve(cbu, products)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedDependencies {
    pub cbu_id: Uuid,
    pub product_ids: Vec<Uuid>,
    pub services: Vec<Service>,
    pub resource_types: Vec<ResourceTypeWithDictionary>,
}

/// Service member resolved from one or more product-service catalogue edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Service {
    pub service_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub service_code: Option<String>,
    pub service_category: Option<String>,
    pub sla_definition: Option<JsonValue>,
    pub is_active: Option<bool>,
    pub lifecycle_tags: Option<Vec<String>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub source_product_ids: Vec<Uuid>,
}

/// Resource type plus the dictionary attributes required by its loaded SRDEF.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceTypeWithDictionary {
    pub srdef_id: String,
    pub resource_id: Option<Uuid>,
    pub code: String,
    pub name: String,
    pub resource_type: String,
    pub purpose: Option<String>,
    pub provisioning_strategy: String,
    pub owner: String,
    pub depends_on: Vec<String>,
    pub parameters: JsonValue,
    pub triggered_by_services: Vec<String>,
    pub source_product_ids: Vec<Uuid>,
    pub dictionary: Vec<ResourceDictionaryAttribute>,
}

/// Dictionary attribute required by a resolved resource type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceDictionaryAttribute {
    pub attr_id: String,
    pub attr_uuid: Option<Uuid>,
    pub requirement: String,
    pub source_policy: Vec<String>,
    pub constraints: JsonValue,
    pub evidence_policy: JsonValue,
    pub default_value: Option<JsonValue>,
    pub condition: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct ProductServiceEdgeRow {
    product_id: Uuid,
    service_id: Uuid,
    name: String,
    description: Option<String>,
    service_code: Option<String>,
    service_category: Option<String>,
    sla_definition: Option<JsonValue>,
    is_active: Option<bool>,
    lifecycle_tags: Option<Vec<String>>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    configuration: JsonValue,
}

#[derive(Debug, Clone, FromRow)]
struct ConditionalProductServiceEdgeRow {
    product_id: Uuid,
    service_id: Uuid,
    name: String,
    description: Option<String>,
    service_code: Option<String>,
    service_category: Option<String>,
    sla_definition: Option<JsonValue>,
    is_active: Option<bool>,
    lifecycle_tags: Option<Vec<String>>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    configuration: JsonValue,
    predicate_dsl: String,
}

impl ConditionalProductServiceEdgeRow {
    fn into_edge(self) -> ProductServiceEdgeRow {
        ProductServiceEdgeRow {
            product_id: self.product_id,
            service_id: self.service_id,
            name: self.name,
            description: self.description,
            service_code: self.service_code,
            service_category: self.service_category,
            sla_definition: self.sla_definition,
            is_active: self.is_active,
            lifecycle_tags: self.lifecycle_tags,
            created_at: self.created_at,
            updated_at: self.updated_at,
            configuration: self.configuration,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
struct CbuProfileRow {
    jurisdiction: Option<String>,
    client_type: Option<String>,
    cbu_category: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct MatrixProfileRow {
    market: Option<String>,
    currencies: Option<Vec<String>>,
    instrument_class: Option<String>,
    counterparty_name: Option<String>,
    is_held: Option<bool>,
    is_traded: Option<bool>,
}

#[derive(Debug, Clone)]
struct CbuResolveProfile {
    jurisdiction: Option<String>,
    client_type: Option<String>,
    cbu_category: Option<String>,
    status: Option<String>,
    markets: BTreeSet<String>,
    currencies: BTreeSet<String>,
    instrument_classes: BTreeSet<String>,
    counterparties: BTreeSet<String>,
    is_held: Option<bool>,
    is_traded: Option<bool>,
}

#[derive(Debug, Clone, FromRow)]
struct ResourceIdRow {
    srdef_id: String,
    resource_id: Uuid,
}

#[derive(Debug, Clone)]
struct ResourceCandidate {
    srdef_id: String,
    parameters: JsonValue,
    triggered_by_services: BTreeSet<String>,
    source_product_ids: BTreeSet<Uuid>,
}

/// Resolve product catalogue membership into services and resource dictionaries without writes.
///
/// This function reads the product-service catalogue and the in-memory SRDEF
/// registry, but it does not create subscriptions, service intents, discovery
/// rows, unified attribute rows, delivery rows, readiness rows, or CBU state
/// updates.
///
/// # Examples
///
/// ```no_run
/// # async fn example(
/// #     pool: &sqlx::PgPool,
/// #     cbu_id: uuid::Uuid,
/// #     product_id: uuid::Uuid,
/// # ) -> anyhow::Result<()> {
/// let resolved = ob_poc::service_resources::resolve(pool, cbu_id, &[product_id]).await?;
/// assert_eq!(resolved.cbu_id, cbu_id);
/// # Ok(())
/// # }
/// ```
pub async fn resolve(
    pool: &PgPool,
    cbu_id: Uuid,
    product_ids: &[Uuid],
) -> Result<ResolvedDependencies> {
    let profile = load_cbu_profile(pool, cbu_id).await?;
    let registry = load_srdefs_from_config()?;
    resolve_with_registry(pool, &registry, cbu_id, product_ids, &profile).await
}

async fn resolve_with_registry(
    pool: &PgPool,
    registry: &SrdefRegistry,
    cbu_id: Uuid,
    product_ids: &[Uuid],
    profile: &CbuResolveProfile,
) -> Result<ResolvedDependencies> {
    let mut normalized_product_ids: Vec<Uuid> = product_ids.to_vec();
    normalized_product_ids.sort_unstable();
    normalized_product_ids.dedup();

    if normalized_product_ids.is_empty() {
        return Ok(ResolvedDependencies {
            cbu_id,
            product_ids: normalized_product_ids,
            services: Vec::new(),
            resource_types: Vec::new(),
        });
    }

    let mut edges = load_product_service_edges(pool, &normalized_product_ids).await?;
    edges.extend(
        load_conditional_product_service_edges(pool, &normalized_product_ids, profile).await?,
    );
    let services = services_from_edges(&edges);
    let resource_types = resource_types_from_edges(pool, registry, &edges).await?;

    Ok(ResolvedDependencies {
        cbu_id,
        product_ids: normalized_product_ids,
        services,
        resource_types,
    })
}

async fn load_cbu_profile(pool: &PgPool, cbu_id: Uuid) -> Result<CbuResolveProfile> {
    let cbu = sqlx::query_as::<_, CbuProfileRow>(
        r#"
        SELECT jurisdiction, client_type, cbu_category, status
        FROM "ob-poc".cbus
        WHERE cbu_id = $1
        "#,
    )
    .bind(cbu_id)
    .fetch_optional(pool)
    .await
    .context("failed to load CBU profile for service-resource resolve")?
    .ok_or_else(|| anyhow!("CBU not found: {cbu_id}"))?;

    let matrix_rows = sqlx::query_as::<_, MatrixProfileRow>(
        r#"
        SELECT market, currencies, instrument_class, counterparty_name, is_held, is_traded
        FROM "ob-poc".v_cbu_matrix_effective
        WHERE cbu_id = $1
        "#,
    )
    .bind(cbu_id)
    .fetch_all(pool)
    .await
    .context("failed to load CBU matrix profile for service-resource resolve")?;

    let mut markets = BTreeSet::new();
    let mut currencies = BTreeSet::new();
    let mut instrument_classes = BTreeSet::new();
    let mut counterparties = BTreeSet::new();
    let mut is_held = None;
    let mut is_traded = None;

    for row in matrix_rows {
        insert_profile_value(&mut markets, row.market);
        if let Some(row_currencies) = row.currencies {
            for currency in row_currencies {
                insert_profile_value(&mut currencies, Some(currency));
            }
        }
        insert_profile_value(&mut instrument_classes, row.instrument_class);
        insert_profile_value(&mut counterparties, row.counterparty_name);
        is_held = merge_profile_bool(is_held, row.is_held);
        is_traded = merge_profile_bool(is_traded, row.is_traded);
    }

    Ok(CbuResolveProfile {
        jurisdiction: cbu.jurisdiction,
        client_type: cbu.client_type,
        cbu_category: cbu.cbu_category,
        status: cbu.status,
        markets,
        currencies,
        instrument_classes,
        counterparties,
        is_held,
        is_traded,
    })
}

async fn load_product_service_edges(
    pool: &PgPool,
    product_ids: &[Uuid],
) -> Result<Vec<ProductServiceEdgeRow>> {
    sqlx::query_as::<_, ProductServiceEdgeRow>(
        r#"
        SELECT
            ps.product_id,
            s.service_id,
            s.name,
            s.description,
            s.service_code,
            s.service_category,
            s.sla_definition,
            s.is_active,
            s.lifecycle_tags,
            s.created_at,
            s.updated_at,
            COALESCE(ps.configuration, '{}'::jsonb) AS configuration
        FROM "ob-poc".product_services ps
        JOIN "ob-poc".services s ON s.service_id = ps.service_id
        WHERE ps.product_id = ANY($1)
          AND COALESCE(s.is_active, TRUE)
        ORDER BY ps.product_id,
                 ps.display_order NULLS LAST,
                 s.service_code NULLS LAST,
                 s.name,
                 s.service_id
        "#,
    )
    .bind(product_ids)
    .fetch_all(pool)
    .await
    .context("failed to load product-service catalogue edges")
}

async fn load_conditional_product_service_edges(
    pool: &PgPool,
    product_ids: &[Uuid],
    profile: &CbuResolveProfile,
) -> Result<Vec<ProductServiceEdgeRow>> {
    let rows = sqlx::query_as::<_, ConditionalProductServiceEdgeRow>(
        r#"
        SELECT
            psc.product_id,
            s.service_id,
            s.name,
            s.description,
            s.service_code,
            s.service_category,
            s.sla_definition,
            s.is_active,
            s.lifecycle_tags,
            s.created_at,
            s.updated_at,
            COALESCE(psc.configuration, '{}'::jsonb) AS configuration,
            psc.predicate_dsl
        FROM "ob-poc".product_service_conditions psc
        JOIN "ob-poc".services s ON s.service_id = psc.service_id
        WHERE psc.product_id = ANY($1)
          AND psc.service_id IS NOT NULL
          AND psc.lifecycle_status = 'active'
          AND NULLIF(BTRIM(psc.predicate_dsl), '') IS NOT NULL
          AND COALESCE(s.is_active, TRUE)
        ORDER BY psc.product_id,
                 psc.display_order NULLS LAST,
                 s.service_code NULLS LAST,
                 s.name,
                 s.service_id
        "#,
    )
    .bind(product_ids)
    .fetch_all(pool)
    .await
    .context("failed to load conditional product-service catalogue edges")?;

    let mut edges = Vec::new();
    for row in rows {
        if evaluate_predicate_dsl(&row.predicate_dsl, profile)? {
            edges.push(row.into_edge());
        }
    }
    Ok(edges)
}

fn services_from_edges(edges: &[ProductServiceEdgeRow]) -> Vec<Service> {
    let mut services: BTreeMap<Uuid, Service> = BTreeMap::new();

    for edge in edges {
        services
            .entry(edge.service_id)
            .and_modify(|service| {
                if !service.source_product_ids.contains(&edge.product_id) {
                    service.source_product_ids.push(edge.product_id);
                    service.source_product_ids.sort_unstable();
                }
            })
            .or_insert_with(|| Service {
                service_id: edge.service_id,
                name: edge.name.clone(),
                description: edge.description.clone(),
                service_code: edge.service_code.clone(),
                service_category: edge.service_category.clone(),
                sla_definition: edge.sla_definition.clone(),
                is_active: edge.is_active,
                lifecycle_tags: edge.lifecycle_tags.clone(),
                created_at: edge.created_at,
                updated_at: edge.updated_at,
                source_product_ids: vec![edge.product_id],
            });
    }

    services.into_values().collect()
}

async fn resource_types_from_edges(
    pool: &PgPool,
    registry: &SrdefRegistry,
    edges: &[ProductServiceEdgeRow],
) -> Result<Vec<ResourceTypeWithDictionary>> {
    let mut direct_candidates: BTreeMap<String, ResourceCandidate> = BTreeMap::new();
    let mut direct_srdef_ids = BTreeSet::new();

    for edge in edges {
        let service_code = edge.service_code.as_deref().unwrap_or(&edge.name);
        let triggered_srdefs = registry.get_by_service(service_code);

        for srdef in triggered_srdefs {
            direct_srdef_ids.insert(srdef.srdef_id.clone());
            for parameters in extract_srdef_parameters(srdef, &edge.configuration)? {
                let parameters_key = serde_json::to_string(&parameters)?;
                let candidate_key = format!("{}:{parameters_key}", srdef.srdef_id);
                direct_candidates
                    .entry(candidate_key)
                    .and_modify(|candidate| {
                        candidate
                            .triggered_by_services
                            .insert(service_code.to_string());
                        candidate.source_product_ids.insert(edge.product_id);
                    })
                    .or_insert_with(|| {
                        let mut triggered_by_services = BTreeSet::new();
                        triggered_by_services.insert(service_code.to_string());
                        let mut source_product_ids = BTreeSet::new();
                        source_product_ids.insert(edge.product_id);
                        ResourceCandidate {
                            srdef_id: srdef.srdef_id.clone(),
                            parameters,
                            triggered_by_services,
                            source_product_ids,
                        }
                    });
            }
        }
    }

    let mut candidates = direct_candidates;
    let direct_ids: Vec<String> = direct_srdef_ids.into_iter().collect();
    let topo_sorted = registry.topo_sort(&direct_ids)?;

    for srdef_id in &topo_sorted {
        if !candidates
            .values()
            .any(|candidate| &candidate.srdef_id == srdef_id)
        {
            candidates.insert(
                format!("{srdef_id}:{{}}"),
                ResourceCandidate {
                    srdef_id: srdef_id.clone(),
                    parameters: JsonValue::Object(serde_json::Map::new()),
                    triggered_by_services: BTreeSet::new(),
                    source_product_ids: BTreeSet::new(),
                },
            );
        }
    }

    let all_srdef_ids: Vec<String> = candidates
        .values()
        .map(|candidate| candidate.srdef_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let resource_ids = load_resource_ids(pool, &all_srdef_ids).await?;
    let topo_index: HashMap<&str, usize> = topo_sorted
        .iter()
        .enumerate()
        .map(|(index, srdef_id)| (srdef_id.as_str(), index))
        .collect();

    let mut output = Vec::new();
    for candidate in candidates.into_values() {
        let Some(srdef) = registry.get(&candidate.srdef_id) else {
            continue;
        };
        output.push(resource_type_from_candidate(
            srdef,
            candidate,
            resource_ids.get(&srdef.srdef_id).copied(),
        ));
    }

    output.sort_by(|left, right| {
        let left_index = topo_index
            .get(left.srdef_id.as_str())
            .copied()
            .unwrap_or(usize::MAX);
        let right_index = topo_index
            .get(right.srdef_id.as_str())
            .copied()
            .unwrap_or(usize::MAX);

        left_index
            .cmp(&right_index)
            .then_with(|| left.srdef_id.cmp(&right.srdef_id))
            .then_with(|| {
                serde_json::to_string(&left.parameters)
                    .unwrap_or_default()
                    .cmp(&serde_json::to_string(&right.parameters).unwrap_or_default())
            })
    });

    Ok(output)
}

async fn load_resource_ids(pool: &PgPool, srdef_ids: &[String]) -> Result<HashMap<String, Uuid>> {
    if srdef_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, ResourceIdRow>(
        r#"
        SELECT srdef_id, resource_id
        FROM "ob-poc".service_resource_types
        WHERE srdef_id = ANY($1)
        "#,
    )
    .bind(srdef_ids)
    .fetch_all(pool)
    .await
    .context("failed to load resource type identifiers for resolved SRDEFs")?;

    Ok(rows
        .into_iter()
        .map(|row| (row.srdef_id, row.resource_id))
        .collect())
}

fn resource_type_from_candidate(
    srdef: &LoadedSrdef,
    candidate: ResourceCandidate,
    resource_id: Option<Uuid>,
) -> ResourceTypeWithDictionary {
    ResourceTypeWithDictionary {
        srdef_id: srdef.srdef_id.clone(),
        resource_id,
        code: srdef.code.clone(),
        name: srdef.name.clone(),
        resource_type: srdef.resource_type.clone(),
        purpose: srdef.purpose.clone(),
        provisioning_strategy: srdef.provisioning_strategy.clone(),
        owner: srdef.owner.clone(),
        depends_on: srdef.depends_on.clone(),
        parameters: candidate.parameters,
        triggered_by_services: candidate.triggered_by_services.into_iter().collect(),
        source_product_ids: candidate.source_product_ids.into_iter().collect(),
        dictionary: srdef.attributes.iter().map(dictionary_attribute).collect(),
    }
}

fn dictionary_attribute(attribute: &LoadedSrdefAttribute) -> ResourceDictionaryAttribute {
    ResourceDictionaryAttribute {
        attr_id: attribute.attr_id.clone(),
        attr_uuid: attribute.attr_uuid,
        requirement: attribute.requirement.clone(),
        source_policy: attribute.source_policy.clone(),
        constraints: attribute.constraints.clone(),
        evidence_policy: attribute.evidence_policy.clone(),
        default_value: attribute.default_value.clone(),
        condition: attribute.condition.clone(),
        description: attribute.description.clone(),
    }
}

fn insert_profile_value(target: &mut BTreeSet<String>, value: Option<String>) {
    if let Some(value) = value {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            target.insert(trimmed.to_string());
        }
    }
}

fn merge_profile_bool(current: Option<bool>, next: Option<bool>) -> Option<bool> {
    match (current, next) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), Some(false)) | (Some(false), None) | (None, Some(false)) => Some(false),
        (None, None) => None,
    }
}

fn evaluate_predicate_dsl(predicate: &str, profile: &CbuResolveProfile) -> Result<bool> {
    let predicate = strip_wrapping_parens(predicate.trim());
    if predicate.is_empty() {
        return Ok(false);
    }

    let or_terms = split_logical(predicate, "or", "||");
    if or_terms.len() > 1 {
        for term in or_terms {
            if evaluate_predicate_dsl(term, profile)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    let and_terms = split_logical(predicate, "and", "&&");
    if and_terms.len() > 1 {
        for term in and_terms {
            if !evaluate_predicate_dsl(term, profile)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }

    evaluate_predicate_clause(predicate, profile)
}

fn evaluate_predicate_clause(predicate: &str, profile: &CbuResolveProfile) -> Result<bool> {
    let predicate = strip_wrapping_parens(predicate.trim());
    if let Some(inner) = strip_word_prefix(predicate, "not") {
        return Ok(!evaluate_predicate_dsl(inner, profile)?);
    }

    if let Some((field, values)) = split_operator(predicate, " in ") {
        let values = parse_list_values(values)?;
        return Ok(profile.field_values(field).iter().any(|profile_value| {
            values
                .iter()
                .any(|condition_value| value_eq(profile_value, condition_value))
        }));
    }

    if let Some((field, expected)) = split_operator(predicate, " contains ") {
        let expected = parse_literal(expected);
        return Ok(profile
            .field_values(field)
            .iter()
            .any(|profile_value| value_eq(profile_value, &expected)));
    }

    if let Some((field, expected)) = split_operator(predicate, "!=") {
        let expected = parse_literal(expected);
        let values = profile.field_values(field);
        return Ok(!values.is_empty()
            && values
                .iter()
                .all(|profile_value| !value_eq(profile_value, &expected)));
    }

    if let Some((field, expected)) =
        split_operator(predicate, "==").or_else(|| split_operator(predicate, "="))
    {
        let expected = parse_literal(expected);
        return Ok(profile
            .field_values(field)
            .iter()
            .any(|profile_value| value_eq(profile_value, &expected)));
    }

    profile
        .field_bool(predicate)
        .ok_or_else(|| anyhow!("unsupported product-service predicate_dsl clause: {predicate}"))
}

impl CbuResolveProfile {
    fn field_values(&self, field: &str) -> Vec<String> {
        match normalize_field(field).as_str() {
            "jurisdiction" => option_value(&self.jurisdiction),
            "client_type" => option_value(&self.client_type),
            "cbu_category" => option_value(&self.cbu_category),
            "status" | "operational_status" | "disposition_status" => option_value(&self.status),
            "market" | "markets" => set_values(&self.markets),
            "currency" | "currencies" => set_values(&self.currencies),
            "instrument_class" | "instrument_classes" => set_values(&self.instrument_classes),
            "counterparty" | "counterparty_name" | "counterparties" => {
                set_values(&self.counterparties)
            }
            "is_held" => bool_value(self.is_held),
            "is_traded" => bool_value(self.is_traded),
            _ => Vec::new(),
        }
    }

    fn field_bool(&self, field: &str) -> Option<bool> {
        match normalize_field(field).as_str() {
            "is_held" => self.is_held,
            "is_traded" => self.is_traded,
            _ => None,
        }
    }
}

fn option_value(value: &Option<String>) -> Vec<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_string()])
        .unwrap_or_default()
}

fn set_values(values: &BTreeSet<String>) -> Vec<String> {
    values.iter().cloned().collect()
}

fn bool_value(value: Option<bool>) -> Vec<String> {
    value
        .map(|value| vec![value.to_string()])
        .unwrap_or_default()
}

fn value_eq(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
}

fn normalize_field(field: &str) -> String {
    field
        .trim()
        .trim_start_matches("cbu.")
        .trim_start_matches("cbus.")
        .replace('-', "_")
        .to_ascii_lowercase()
}

fn parse_literal(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn parse_list_values(value: &str) -> Result<Vec<String>> {
    let trimmed = value.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .or_else(|| {
            trimmed
                .strip_prefix('(')
                .and_then(|value| value.strip_suffix(')'))
        })
        .ok_or_else(|| anyhow!("predicate_dsl in-list must use [] or (): {value}"))?;

    Ok(split_csv_values(inner)
        .into_iter()
        .map(|value| parse_literal(&value))
        .filter(|value| !value.is_empty())
        .collect())
}

fn split_csv_values(input: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut current = String::new();
    let mut quote = None;

    for ch in input.chars() {
        match (ch, quote) {
            ('\'' | '"', None) => {
                quote = Some(ch);
                current.push(ch);
            }
            (_, Some(active)) if ch == active => {
                quote = None;
                current.push(ch);
            }
            (',', None) => {
                output.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if !current.trim().is_empty() {
        output.push(current.trim().to_string());
    }

    output
}

fn split_operator<'a>(input: &'a str, operator: &str) -> Option<(&'a str, &'a str)> {
    find_operator(input, operator).map(|index| {
        let (left, right_with_operator) = input.split_at(index);
        let right = &right_with_operator[operator.len()..];
        (left.trim(), right.trim())
    })
}

fn find_operator(input: &str, operator: &str) -> Option<usize> {
    let operator_lower = operator.to_ascii_lowercase();
    for (index, _) in input.char_indices() {
        if input[index..]
            .to_ascii_lowercase()
            .starts_with(&operator_lower)
        {
            return Some(index);
        }
    }
    None
}

fn split_logical<'a>(input: &'a str, word: &str, symbol: &str) -> Vec<&'a str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut depth = 0i32;

    for (index, ch) in input.char_indices() {
        match (ch, quote) {
            ('\'' | '"', None) => {
                quote = Some(ch);
                continue;
            }
            (_, Some(active)) if ch == active => {
                quote = None;
                continue;
            }
            _ => {}
        }

        if quote.is_some() {
            continue;
        }

        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            _ => {}
        }

        if depth == 0 && input[index..].starts_with(symbol) {
            parts.push(input[start..index].trim());
            start = index + symbol.len();
            continue;
        }

        if depth == 0 && is_word_at(input, index, word) {
            parts.push(input[start..index].trim());
            start = index + word.len();
        }
    }

    if start == 0 {
        return vec![input.trim()];
    }

    parts.push(input[start..].trim());
    parts.into_iter().filter(|part| !part.is_empty()).collect()
}

fn is_word_at(input: &str, index: usize, word: &str) -> bool {
    let Some(candidate) = input.get(index..index + word.len()) else {
        return false;
    };
    if !candidate.eq_ignore_ascii_case(word) {
        return false;
    }

    let before = input[..index].chars().next_back();
    let after = input[index + word.len()..].chars().next();
    !is_identifier_char(before) && !is_identifier_char(after)
}

fn is_identifier_char(ch: Option<char>) -> bool {
    ch.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn strip_word_prefix<'a>(input: &'a str, word: &str) -> Option<&'a str> {
    if is_word_at(input, 0, word) {
        Some(input[word.len()..].trim())
    } else {
        None
    }
}

fn strip_wrapping_parens(mut input: &str) -> &str {
    loop {
        let trimmed = input.trim();
        let Some(inner) = trimmed
            .strip_prefix('(')
            .and_then(|value| value.strip_suffix(')'))
        else {
            return trimmed;
        };
        if parens_wrap_entire_expression(trimmed) {
            input = inner;
        } else {
            return trimmed;
        }
    }
}

fn parens_wrap_entire_expression(input: &str) -> bool {
    let mut quote = None;
    let mut depth = 0i32;

    for (index, ch) in input.char_indices() {
        match (ch, quote) {
            ('\'' | '"', None) => {
                quote = Some(ch);
                continue;
            }
            (_, Some(active)) if ch == active => {
                quote = None;
                continue;
            }
            _ => {}
        }

        if quote.is_some() {
            continue;
        }

        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && index < input.len() - 1 {
                    return false;
                }
            }
            _ => {}
        }
    }

    depth == 0
}
