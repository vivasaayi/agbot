//! Marketplace route handlers (equipment/produce marketplace domain).
//!
//! Thin HTTP wrappers over the `interop`/marketplace domain modules: accounts,
//! catalog items, listings, inventory, orders, fulfillment, ratings, demand
//! forecasts, and org reports. Extracted verbatim from routes.rs; shared helpers
//! and the `marketplace_*_error` mappers are reached from the parent module.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn create_marketplace_account(
    State(state): State<AppState>,
    Json(request): Json<MarketplaceAccountCreateRequest>,
) -> AppResult<Json<MarketplaceAccountRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id.clone()))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let org_exists = marketplace_org_exists(&state, &org_id).await?;
    let record = build_marketplace_account_record(
        request,
        org_exists,
        format!("marketplace-account-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(marketplace_account_error)?;
    insert_marketplace_account_record(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_marketplace_accounts(
    Query(query): Query<MarketplaceAccountListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceAccountRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace accounts".to_string(),
        )
    })?;
    let party_type = query
        .party_type
        .map(|party_type| party_type.as_str().to_string());
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT account_id, org_id, party_type, role_refs_json, status, created_at, updated_at
        FROM marketplace_accounts
        WHERE (?1 IS NULL OR org_id = ?1)
          AND (?2 IS NULL OR party_type = ?2)
          AND (?3 IS NULL OR status = ?3)
        ORDER BY created_at ASC, account_id ASC
        "#,
    )
    .bind(org_id)
    .bind(party_type)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_account_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_marketplace_account(
    Path(account_id): Path<String>,
    Query(query): Query<MarketplaceAccountScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceAccountRecord>> {
    let org_id = normalize_optional_text(query.org_id.clone())
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let account = load_marketplace_account(&state, &account_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if account.org_id != org_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(account))
}

pub async fn update_marketplace_account_status(
    Path(account_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<MarketplaceAccountStatusRequest>,
) -> AppResult<Json<MarketplaceAccountRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let account = load_marketplace_account(&state, &account_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if account.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let updated =
        transition_marketplace_account_status(&account, request.status, current_record_timestamp())
            .map_err(marketplace_account_error)?;
    update_marketplace_account_record(&state, &updated).await?;

    Ok(Json(updated))
}

pub async fn create_marketplace_catalog_item(
    State(state): State<AppState>,
    Json(request): Json<MarketplaceCatalogItemCreateRequest>,
) -> AppResult<Json<MarketplaceCatalogItemRecord>> {
    let owner_account_id = normalize_optional_text(Some(request.owner_account_id.clone()))
        .ok_or_else(|| {
            AppError::BadRequest("marketplace owner_account_id is required".to_string())
        })?;
    let owner_account = load_marketplace_account(&state, &owner_account_id).await?;
    let record = build_marketplace_catalog_item_record(
        request,
        owner_account.as_ref(),
        format!("marketplace-item-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(marketplace_catalog_error)?;
    insert_marketplace_catalog_item_record(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_marketplace_catalog_items(
    Query(query): Query<MarketplaceCatalogListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceCatalogItemRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace catalog".to_string(),
        )
    })?;
    let kind = query.kind.map(|kind| kind.as_str().to_string());
    let category = query.category.map(|category| category.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT item_id, org_id, kind, category, name, unit_of_measure, owner_account_id, created_at
        FROM marketplace_catalog_items
        WHERE (?1 IS NULL OR org_id = ?1)
          AND (?2 IS NULL OR kind = ?2)
          AND (?3 IS NULL OR category = ?3)
        ORDER BY created_at ASC, item_id ASC
        "#,
    )
    .bind(org_id)
    .bind(kind)
    .bind(category)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_catalog_item_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_marketplace_catalog_item(
    Path(item_id): Path<String>,
    Query(query): Query<MarketplaceCatalogScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceCatalogItemRecord>> {
    let org_id = normalize_optional_text(query.org_id.clone())
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let item = load_marketplace_catalog_item(&state, &item_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if item.org_id != org_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(item))
}

pub async fn get_marketplace_portal_entry(
    Query(query): Query<MarketplacePortalEntryQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplacePortalEntry>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let account_id = normalize_optional_text(query.account_id).ok_or_else(|| {
        AppError::BadRequest("account_id query parameter is required".to_string())
    })?;
    let account = load_marketplace_account(&state, &account_id).await?;
    let entry = build_marketplace_portal_entry(account.as_ref(), org_id)
        .map_err(marketplace_portal_entry_error)?;

    Ok(Json(entry))
}

pub async fn publish_marketplace_listing(
    State(state): State<AppState>,
    Json(request): Json<MarketplaceListingPublishRequest>,
) -> AppResult<Json<MarketplaceListingRecord>> {
    let item_id = normalize_optional_text(Some(request.item_id.clone()))
        .ok_or_else(|| AppError::BadRequest("marketplace item_id is required".to_string()))?;
    let catalog_item = load_marketplace_catalog_item(&state, &item_id).await?;
    let record = publish_marketplace_listing_record(
        request,
        catalog_item.as_ref(),
        format!("marketplace-listing-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(marketplace_listing_error)?;
    insert_marketplace_listing_record(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_marketplace_listings(
    Query(query): Query<MarketplaceListingListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceListingRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace listings".to_string(),
        )
    })?;
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT listing_id, item_id, org_id, price, currency, available_qty,
               window_from, window_to, status, created_at, updated_at
        FROM marketplace_listings
        WHERE (?1 IS NULL OR org_id = ?1)
          AND (?2 IS NULL OR status = ?2)
        ORDER BY created_at ASC, listing_id ASC
        "#,
    )
    .bind(org_id)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_listing_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_marketplace_listing(
    Path(listing_id): Path<String>,
    Query(query): Query<MarketplaceListingScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceListingRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let listing = load_marketplace_listing(&state, &listing_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if listing.org_id != org_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(listing))
}

pub async fn close_marketplace_listing(
    Path(listing_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<MarketplaceListingCloseRequest>,
) -> AppResult<Json<MarketplaceListingRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let listing = load_marketplace_listing(&state, &listing_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if listing.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let updated = close_marketplace_listing_record(&listing, current_record_timestamp())
        .map_err(marketplace_listing_error)?;
    update_marketplace_listing_record(&state, &updated).await?;

    Ok(Json(updated))
}

pub async fn upsert_marketplace_inventory(
    State(state): State<AppState>,
    Json(request): Json<MarketplaceInventoryUpsertRequest>,
) -> AppResult<Json<MarketplaceInventoryRecord>> {
    let item_id = normalize_optional_text(Some(request.item_id.clone()))
        .ok_or_else(|| AppError::BadRequest("marketplace item_id is required".to_string()))?;
    let catalog_item = load_marketplace_catalog_item(&state, &item_id).await?;
    let record = build_marketplace_inventory_record(
        request,
        catalog_item.as_ref(),
        format!("marketplace-inventory-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(marketplace_inventory_error)?;
    upsert_marketplace_inventory_record(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_marketplace_inventory(
    Query(query): Query<MarketplaceInventoryListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceInventoryRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace inventory".to_string(),
        )
    })?;
    let rows = sqlx::query(
        r#"
        SELECT inventory_id, item_id, org_id, on_hand, reserved, updated_at
        FROM marketplace_inventory
        WHERE org_id = ?1
        ORDER BY item_id ASC, inventory_id ASC
        "#,
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_inventory_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_marketplace_inventory(
    Path(inventory_id): Path<String>,
    Query(query): Query<MarketplaceInventoryScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceInventoryRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let inventory = load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if inventory.org_id != org_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(inventory))
}

pub async fn reserve_marketplace_inventory_endpoint(
    Path(inventory_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<MarketplaceInventoryAdjustmentRequest>,
) -> AppResult<Json<MarketplaceInventoryRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let inventory = load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if inventory.org_id != org_id {
        return Err(AppError::NotFound);
    }
    reserve_marketplace_inventory(&inventory, request.qty, current_record_timestamp())
        .map_err(marketplace_inventory_error)?;
    update_marketplace_inventory_reserve(&state, &inventory_id, &org_id, request.qty).await?;
    load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn fulfill_marketplace_inventory_endpoint(
    Path(inventory_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<MarketplaceInventoryAdjustmentRequest>,
) -> AppResult<Json<MarketplaceInventoryRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let inventory = load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if inventory.org_id != org_id {
        return Err(AppError::NotFound);
    }
    fulfill_marketplace_inventory(&inventory, request.qty, current_record_timestamp())
        .map_err(marketplace_inventory_error)?;
    update_marketplace_inventory_fulfill(&state, &inventory_id, &org_id, request.qty).await?;
    load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn release_marketplace_inventory_endpoint(
    Path(inventory_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<MarketplaceInventoryAdjustmentRequest>,
) -> AppResult<Json<MarketplaceInventoryRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let inventory = load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if inventory.org_id != org_id {
        return Err(AppError::NotFound);
    }
    release_marketplace_inventory(&inventory, request.qty, current_record_timestamp())
        .map_err(marketplace_inventory_error)?;
    update_marketplace_inventory_release(&state, &inventory_id, &org_id, request.qty).await?;
    load_marketplace_inventory(&state, &inventory_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn place_marketplace_order(
    State(state): State<AppState>,
    Json(request): Json<MarketplaceOrderCreateRequest>,
) -> AppResult<Json<MarketplaceOrderRecord>> {
    let listing_ref = normalize_optional_text(Some(request.listing_ref.clone()))
        .ok_or_else(|| AppError::BadRequest("marketplace listing_ref is required".to_string()))?;
    let buyer_account_id = normalize_optional_text(Some(request.buyer_account_id.clone()))
        .ok_or_else(|| {
            AppError::BadRequest("marketplace buyer_account_id is required".to_string())
        })?;
    let listing = load_marketplace_listing(&state, &listing_ref).await?;
    let buyer_account = load_marketplace_account(&state, &buyer_account_id).await?;
    let (order, audit) = place_marketplace_order_record(
        request,
        listing.as_ref(),
        buyer_account.as_ref(),
        format!("marketplace-order-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(marketplace_order_error)?;
    let listing = listing.ok_or(AppError::NotFound)?;
    let inventory = load_marketplace_inventory_by_item(&state, &listing.item_id, &order.org_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("marketplace inventory is required".to_string()))?;
    reserve_marketplace_inventory(&inventory, order.qty, current_record_timestamp())
        .map_err(marketplace_inventory_error)?;
    update_marketplace_inventory_reserve(&state, &inventory.inventory_id, &order.org_id, order.qty)
        .await?;
    insert_marketplace_order_record(&state, &order).await?;
    insert_marketplace_order_audit_record(&state, &audit).await?;

    Ok(Json(order))
}

pub async fn list_marketplace_orders(
    Query(query): Query<MarketplaceOrderListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceOrderRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace orders".to_string(),
        )
    })?;
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT order_id, org_id, listing_ref, buyer_account_id, qty, line_total,
               status, created_at, updated_at
        FROM marketplace_orders
        WHERE org_id = ?1
          AND (?2 IS NULL OR status = ?2)
        ORDER BY created_at ASC, order_id ASC
        "#,
    )
    .bind(org_id)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_order_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_marketplace_order(
    Path(order_id): Path<String>,
    Query(query): Query<MarketplaceOrderScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceOrderRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let order = load_marketplace_order(&state, &order_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if order.org_id != org_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(order))
}

pub async fn transition_marketplace_order(
    Path(order_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<MarketplaceOrderTransitionRequest>,
) -> AppResult<Json<MarketplaceOrderRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let order = load_marketplace_order(&state, &order_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if order.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let (updated, audit) = transition_marketplace_order_status(
        &order,
        request.status,
        request.actor_id,
        current_record_timestamp(),
    )
    .map_err(marketplace_order_error)?;
    if updated.status == MarketplaceOrderStatus::Fulfilled {
        let listing = load_marketplace_listing(&state, &order.listing_ref)
            .await?
            .ok_or(AppError::NotFound)?;
        let inventory = load_marketplace_inventory_by_item(&state, &listing.item_id, &order.org_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("marketplace inventory is required".to_string()))?;
        update_marketplace_inventory_fulfill(
            &state,
            &inventory.inventory_id,
            &order.org_id,
            order.qty,
        )
        .await?;
    } else if updated.status == MarketplaceOrderStatus::Cancelled {
        let listing = load_marketplace_listing(&state, &order.listing_ref)
            .await?
            .ok_or(AppError::NotFound)?;
        let inventory = load_marketplace_inventory_by_item(&state, &listing.item_id, &order.org_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("marketplace inventory is required".to_string()))?;
        update_marketplace_inventory_release(
            &state,
            &inventory.inventory_id,
            &order.org_id,
            order.qty,
        )
        .await?;
    }
    update_marketplace_order_record(&state, &updated).await?;
    insert_marketplace_order_audit_record(&state, &audit).await?;

    Ok(Json(updated))
}

pub async fn list_marketplace_order_audits(
    Path(order_id): Path<String>,
    Query(query): Query<MarketplaceOrderScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceOrderAuditRecord>>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let order = load_marketplace_order(&state, &order_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if order.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let rows = sqlx::query(
        r#"
        SELECT audit_id, order_id, from_status, to_status, actor_id, occurred_at
        FROM marketplace_order_audits
        WHERE order_id = ?1
        ORDER BY occurred_at ASC, audit_id ASC
        "#,
    )
    .bind(order_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_order_audit_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn create_marketplace_fulfillment(
    State(state): State<AppState>,
    Json(request): Json<MarketplaceFulfillmentCreateRequest>,
) -> AppResult<Json<MarketplaceFulfillmentRecord>> {
    let order_ref = normalize_optional_text(Some(request.order_ref.clone())).ok_or_else(|| {
        AppError::BadRequest("marketplace fulfillment order_ref is required".to_string())
    })?;
    let actor_id = normalize_optional_text(Some(request.actor_id.clone())).ok_or_else(|| {
        AppError::BadRequest("marketplace fulfillment actor_id is required".to_string())
    })?;
    let order = load_marketplace_order(&state, &order_ref).await?;
    let (record, audit) = create_marketplace_fulfillment_record(
        request,
        order.as_ref(),
        format!("marketplace-fulfillment-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(marketplace_fulfillment_error)?;
    let order = order.ok_or(AppError::NotFound)?;
    let (fulfilled_order, order_audit) = transition_marketplace_order_status(
        &order,
        MarketplaceOrderStatus::Fulfilled,
        actor_id,
        current_record_timestamp(),
    )
    .map_err(marketplace_order_error)?;
    let listing = load_marketplace_listing(&state, &order.listing_ref)
        .await?
        .ok_or(AppError::NotFound)?;
    let inventory = load_marketplace_inventory_by_item(&state, &listing.item_id, &order.org_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("marketplace inventory is required".to_string()))?;
    update_marketplace_inventory_fulfill(&state, &inventory.inventory_id, &order.org_id, order.qty)
        .await?;
    update_marketplace_order_record(&state, &fulfilled_order).await?;
    insert_marketplace_order_audit_record(&state, &order_audit).await?;
    insert_marketplace_fulfillment_record(&state, &record).await?;
    insert_marketplace_fulfillment_audit_record(&state, &audit).await?;

    Ok(Json(record))
}

pub async fn list_marketplace_fulfillments(
    Query(query): Query<MarketplaceFulfillmentListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceFulfillmentRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace fulfillments".to_string(),
        )
    })?;
    let order_ref = normalize_optional_text(query.order_ref);
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT fulfillment_id, order_ref, org_id, carrier_ref, tracking_ref,
               status, created_at, updated_at
        FROM marketplace_fulfillments
        WHERE org_id = ?1
          AND (?2 IS NULL OR order_ref = ?2)
          AND (?3 IS NULL OR status = ?3)
        ORDER BY created_at ASC, fulfillment_id ASC
        "#,
    )
    .bind(org_id)
    .bind(order_ref)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_fulfillment_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_marketplace_fulfillment(
    Path(fulfillment_id): Path<String>,
    Query(query): Query<MarketplaceFulfillmentScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceFulfillmentRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let fulfillment = load_marketplace_fulfillment(&state, &fulfillment_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if fulfillment.org_id != org_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(fulfillment))
}

pub async fn transition_marketplace_fulfillment(
    Path(fulfillment_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<MarketplaceFulfillmentTransitionRequest>,
) -> AppResult<Json<MarketplaceFulfillmentRecord>> {
    let org_id = normalize_optional_text(Some(request.org_id))
        .ok_or_else(|| AppError::BadRequest("marketplace org_id is required".to_string()))?;
    let fulfillment = load_marketplace_fulfillment(&state, &fulfillment_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if fulfillment.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let (updated, audit) = transition_marketplace_fulfillment_status(
        &fulfillment,
        request.status,
        request.actor_id,
        current_record_timestamp(),
    )
    .map_err(marketplace_fulfillment_error)?;
    update_marketplace_fulfillment_record(&state, &updated).await?;
    insert_marketplace_fulfillment_audit_record(&state, &audit).await?;

    Ok(Json(updated))
}

pub async fn list_marketplace_fulfillment_audits(
    Path(fulfillment_id): Path<String>,
    Query(query): Query<MarketplaceFulfillmentScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceFulfillmentAuditRecord>>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let fulfillment = load_marketplace_fulfillment(&state, &fulfillment_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if fulfillment.org_id != org_id {
        return Err(AppError::NotFound);
    }
    let rows = sqlx::query(
        r#"
        SELECT audit_id, fulfillment_id, from_status, to_status, actor_id, occurred_at
        FROM marketplace_fulfillment_audits
        WHERE fulfillment_id = ?1
        ORDER BY occurred_at ASC, audit_id ASC
        "#,
    )
    .bind(fulfillment_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_fulfillment_audit_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn create_marketplace_rating(
    State(state): State<AppState>,
    Json(request): Json<MarketplaceRatingCreateRequest>,
) -> AppResult<Json<MarketplaceRatingRecord>> {
    let order_ref = normalize_optional_text(Some(request.order_ref.clone())).ok_or_else(|| {
        AppError::BadRequest("marketplace rating order_ref is required".to_string())
    })?;
    let order = load_marketplace_order(&state, &order_ref).await?;
    let participants = if let Some(order) = order.as_ref() {
        marketplace_order_participants(&state, order).await?
    } else {
        Vec::new()
    };
    let existing = load_marketplace_ratings_for_order(&state, &order_ref).await?;
    let record = create_marketplace_rating_record(
        request,
        order.as_ref(),
        &participants,
        &existing,
        format!("marketplace-rating-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(marketplace_rating_error)?;
    insert_marketplace_rating_record(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_marketplace_ratings(
    Query(query): Query<MarketplaceRatingListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceRatingRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace ratings".to_string(),
        )
    })?;
    let order_ref = normalize_optional_text(query.order_ref);
    let ratee_account_id = normalize_optional_text(query.ratee_account_id);
    let rows = sqlx::query(
        r#"
        SELECT rating_id, order_ref, rater_account_id, ratee_account_id,
               score, comment, org_scope, created_at
        FROM marketplace_ratings
        WHERE org_scope = ?1
          AND (?2 IS NULL OR order_ref = ?2)
          AND (?3 IS NULL OR ratee_account_id = ?3)
        ORDER BY created_at ASC, rating_id ASC
        "#,
    )
    .bind(org_id)
    .bind(order_ref)
    .bind(ratee_account_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_rating_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_marketplace_rating_aggregate(
    Path(account_id): Path<String>,
    Query(query): Query<MarketplaceRatingAggregateQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceRatingAggregate>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let ratings = load_marketplace_ratings_for_ratee(&state, &account_id, &org_id).await?;
    let aggregate = aggregate_marketplace_ratings(account_id, org_id, &ratings)
        .map_err(marketplace_rating_error)?;

    Ok(Json(aggregate))
}

pub async fn create_marketplace_demand_forecast(
    State(state): State<AppState>,
    Json(request): Json<MarketplaceDemandForecastRequest>,
) -> AppResult<Json<MarketplaceDemandForecastRecord>> {
    let field_id = normalize_optional_text(Some(request.field_id.clone())).ok_or_else(|| {
        AppError::BadRequest("marketplace demand field_id is required".to_string())
    })?;
    let field = load_field(&state, &field_id).await?;
    let evidence_refs = load_marketplace_demand_evidence_refs(&state, &field_id).await?;
    let record = compute_marketplace_demand_forecast(
        request,
        field.as_ref(),
        evidence_refs,
        format!("marketplace-demand-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(marketplace_demand_forecast_error)?;
    insert_marketplace_demand_forecast_record(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_marketplace_demand_forecasts(
    Query(query): Query<MarketplaceDemandForecastListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MarketplaceDemandForecastRecord>>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace demand forecasts".to_string(),
        )
    })?;
    let field_id = normalize_optional_text(query.field_id);
    let rows = sqlx::query(
        r#"
        SELECT forecast_id, org_id, field_id, item_kind, horizon, value,
               evidence_refs_json, status, uncertainty_low, uncertainty_high,
               method, created_at
        FROM marketplace_demand_forecasts
        WHERE org_id = ?1
          AND (?2 IS NULL OR field_id = ?2)
        ORDER BY created_at ASC, forecast_id ASC
        "#,
    )
    .bind(org_id)
    .bind(field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_marketplace_demand_forecast_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_marketplace_demand_forecast(
    Path(forecast_id): Path<String>,
    Query(query): Query<MarketplaceDemandForecastScopeQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceDemandForecastRecord>> {
    let org_id = normalize_optional_text(query.org_id)
        .ok_or_else(|| AppError::BadRequest("org_id query parameter is required".to_string()))?;
    let forecast = load_marketplace_demand_forecast(&state, &forecast_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if forecast.org_id != org_id {
        return Err(AppError::NotFound);
    }

    Ok(Json(forecast))
}

pub async fn get_marketplace_org_report(
    Query(query): Query<MarketplaceReportQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<MarketplaceOrgReport>> {
    let org_id = normalize_optional_text(query.org_id).ok_or_else(|| {
        AppError::BadRequest(
            "org_id query parameter is required for marketplace report".to_string(),
        )
    })?;
    let from = normalize_optional_text(query.from).ok_or_else(|| {
        AppError::BadRequest("from query parameter is required for marketplace report".to_string())
    })?;
    let to = normalize_optional_text(query.to).ok_or_else(|| {
        AppError::BadRequest("to query parameter is required for marketplace report".to_string())
    })?;
    let orders = load_marketplace_orders_for_org_period(&state, &org_id, &from, &to).await?;
    let listings = load_marketplace_listings_for_org(&state, &org_id).await?;
    let inventory = load_marketplace_inventory_for_org(&state, &org_id).await?;
    let report = assemble_marketplace_org_report(
        MarketplaceOrgReportRequest {
            org_id,
            period: MarketplaceReportPeriod { from, to },
        },
        &orders,
        &listings,
        &inventory,
        current_record_timestamp(),
    )
    .map_err(marketplace_report_error)?;

    Ok(Json(report))
}
