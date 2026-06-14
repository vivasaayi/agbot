use crate::state::{
    ReportFetchTask, ReportGenerateTask, ReportOverlayState, ReportZoneOverlay, TileConfig,
    TileRenderState, TileStatus,
};
use anyhow::{Context, Result};
use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use futures_lite::future;
use shared::schemas::ReportRecord;

pub struct ViewerReportsPlugin;

impl Plugin for ViewerReportsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (poll_report_fetch, poll_report_generate, render_report_zones),
        );
    }
}

pub fn poll_report_fetch(
    mut report_fetch_task: ResMut<ReportFetchTask>,
    mut reports: ResMut<ReportOverlayState>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = report_fetch_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(items) => reports.items = items,
                Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
            }
        } else {
            report_fetch_task.0 = Some(task);
        }
    }
}

pub fn poll_report_generate(
    mut report_generate_task: ResMut<ReportGenerateTask>,
    mut reports: ResMut<ReportOverlayState>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = report_generate_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(report) => {
                    reports.items.insert(0, report);
                    tile_state.status = TileStatus::Ready;
                }
                Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
            }
        } else {
            report_generate_task.0 = Some(task);
        }
    }
}

pub fn start_report_fetch(
    report_fetch_task: &mut ReportFetchTask,
    config: &TileConfig,
) -> Result<()> {
    let scene_id = match &config.scene_id {
        Some(id) => id.clone(),
        None => {
            report_fetch_task.0 = None;
            return Ok(());
        }
    };

    let url = format!("{}/api/scenes/{}/reports", config.base_url, scene_id);
    report_fetch_task.0 = Some(IoTaskPool::get().spawn(async move {
        let response =
            reqwest::blocking::get(&url).with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response
            .bytes()
            .context("failed to read reports response body")?;
        let reports = serde_json::from_slice::<Vec<ReportRecord>>(&bytes)
            .context("failed to decode reports")?;
        Ok(reports)
    }));

    Ok(())
}

pub fn start_report_generate(
    report_generate_task: &mut ReportGenerateTask,
    config: &TileConfig,
    title: String,
) -> Result<()> {
    let scene_id = crate::state::ensure_scene_id(config, "generate reports")?;
    let url = format!("{}/api/scenes/{}/reports", config.base_url, scene_id);
    let payload = serde_json::json!({ "title": title }).to_string();

    report_generate_task.0 = Some(IoTaskPool::get().spawn(async move {
        let client = reqwest::blocking::Client::new();
        let response = client
            .post(&url)
            .header("content-type", "application/json")
            .body(payload)
            .send()
            .with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response
            .bytes()
            .context("failed to read generate report response body")?;
        let report =
            serde_json::from_slice::<ReportRecord>(&bytes).context("failed to decode report")?;
        Ok(report)
    }));

    Ok(())
}

pub fn clear_reports(
    reports: &mut ReportOverlayState,
    report_fetch_task: &mut ReportFetchTask,
    report_generate_task: &mut ReportGenerateTask,
) {
    reports.items.clear();
    reports.zones.clear();
    reports.last_overlay_error = None;
    report_fetch_task.0 = None;
    report_generate_task.0 = None;
}

pub fn render_report_zones(mut gizmos: Gizmos, reports: Res<ReportOverlayState>) {
    for zone in &reports.zones {
        draw_report_zone(&mut gizmos, zone);
    }
}

fn draw_report_zone(gizmos: &mut Gizmos, zone: &ReportZoneOverlay) {
    if zone.world_polygon.len() < 3 {
        return;
    }
    let color = report_zone_color(zone.priority);
    for segment in zone.world_polygon.windows(2) {
        gizmos.line_2d(segment[0], segment[1], color);
    }
    let first = zone.world_polygon[0];
    let last = *zone
        .world_polygon
        .last()
        .expect("world polygon has at least three vertices");
    gizmos.line_2d(last, first, color);
}

fn report_zone_color(priority: shared::schemas::RecommendationPriority) -> Color {
    match priority {
        shared::schemas::RecommendationPriority::Low => Color::srgba(0.35, 0.75, 0.45, 0.9),
        shared::schemas::RecommendationPriority::Medium => Color::srgba(0.95, 0.78, 0.2, 0.9),
        shared::schemas::RecommendationPriority::High => Color::srgba(0.95, 0.42, 0.16, 0.95),
        shared::schemas::RecommendationPriority::Critical => Color::srgba(0.9, 0.12, 0.2, 1.0),
    }
}
