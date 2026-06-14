use crate::state::{
    ReportFetchTask, ReportGenerateTask, ReportOverlayState, TileConfig, TileRenderState,
    TileStatus,
};
use anyhow::{Context, Result};
use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use futures_lite::future;
use shared::schemas::ReportRecord;

pub struct ViewerReportsPlugin;

impl Plugin for ViewerReportsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (poll_report_fetch, poll_report_generate));
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
    report_fetch_task.0 = None;
    report_generate_task.0 = None;
}
