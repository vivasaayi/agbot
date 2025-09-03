use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::world_exploration::{WorldLocation, World3DState};
use crate::city_search::CityDatabase;

pub struct SearchInterfacePlugin;

impl Plugin for SearchInterfacePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<SearchState>()
            .add_systems(Update, handle_search_interface);
    }
}

#[derive(Resource, Default)]
pub struct SearchState {
    pub search_results: Vec<WorldLocation>,
    pub selected_index: Option<usize>,
    pub show_dropdown: bool,
    pub last_query: String,
}

impl SearchState {
    pub fn update_search(&mut self, query: &str, database: &CityDatabase) {
        if query != self.last_query {
            self.search_results = database.search_cities(query);
            self.show_dropdown = !query.is_empty() && !self.search_results.is_empty();
            self.last_query = query.to_string();
            self.selected_index = None;
        }
    }
    
    pub fn select_result(&mut self, index: usize) -> Option<WorldLocation> {
        if index < self.search_results.len() {
            self.selected_index = Some(index);
            Some(self.search_results[index].clone())
        } else {
            None
        }
    }
    
    pub fn clear(&mut self) {
        self.search_results.clear();
        self.show_dropdown = false;
        self.selected_index = None;
        self.last_query.clear();
    }
}

fn handle_search_interface(
    mut contexts: EguiContexts,
    mut search_state: ResMut<SearchState>,
    world_state: Option<ResMut<World3DState>>,
    city_database: Res<CityDatabase>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    // Only handle search if we're in 3D world exploration mode
    let mut world_state = match world_state {
        Some(state) => state,
        None => return, // Not in 3D world exploration mode
    };
    
    let ctx = contexts.ctx_mut();
    
    // Handle keyboard navigation
    if search_state.show_dropdown {
        if keyboard_input.just_pressed(KeyCode::ArrowDown) {
            let max_index = search_state.search_results.len().saturating_sub(1);
            if let Some(ref mut index) = search_state.selected_index {
                *index = (*index + 1).min(max_index);
            } else {
                search_state.selected_index = Some(0);
            }
        }
        
        if keyboard_input.just_pressed(KeyCode::ArrowUp) {
            if let Some(ref mut index) = search_state.selected_index {
                if *index > 0 {
                    *index -= 1;
                } else {
                    search_state.selected_index = None;
                }
            }
        }
        
        if keyboard_input.just_pressed(KeyCode::Enter) {
            if let Some(index) = search_state.selected_index {
                if let Some(location) = search_state.select_result(index) {
                    world_state.selected_location = Some(location.clone());
                    world_state.search_query = location.name;
                    world_state.show_load_button = true;
                    search_state.clear();
                }
            }
        }
        
        if keyboard_input.just_pressed(KeyCode::Escape) {
            search_state.clear();
        }
    }
    
    // Update search results based on current query
    search_state.update_search(&world_state.search_query, &city_database);
    
    // Show search dropdown if there are results
    if search_state.show_dropdown {
        let search_rect = egui::Rect::from_min_size(
            egui::pos2(140.0, 90.0), // Positioned below search bar
            egui::vec2(300.0, 200.0)
        );
        
        egui::Window::new("search_results")
            .title_bar(false)
            .resizable(false)
            .fixed_rect(search_rect)
            .frame(egui::Frame::popup(&ctx.style()))
            .show(ctx, |ui| {
                ui.label(format!("Found {} cities:", search_state.search_results.len()));
                ui.separator();
                
                let mut selected_city = None;
                let mut clear_search = false;
                let mut hovered_index = None;
                
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for (index, city) in search_state.search_results.iter().enumerate() {
                            let is_selected = search_state.selected_index == Some(index);
                            
                            let response = ui.selectable_label(
                                is_selected,
                                format!("{}, {}", city.name, city.country)
                            );
                            
                            if response.clicked() {
                                selected_city = Some(city.clone());
                                clear_search = true;
                            }
                            
                            if response.hovered() {
                                hovered_index = Some(index);
                            }
                        }
                    });
                
                // Apply changes after iteration
                if let Some(city) = selected_city {
                    world_state.selected_location = Some(city.clone());
                    world_state.search_query = city.name.clone();
                    world_state.show_load_button = true;
                }
                
                if clear_search {
                    search_state.clear();
                }
                
                if let Some(index) = hovered_index {
                    search_state.selected_index = Some(index);
                }
            });
    }
}

/// Advanced search with fuzzy matching
pub fn fuzzy_search_cities(query: &str, cities: &[WorldLocation]) -> Vec<(WorldLocation, f32)> {
    if query.is_empty() {
        return Vec::new();
    }
    
    let query_lower = query.to_lowercase();
    let mut scored_results: Vec<(WorldLocation, f32)> = cities
        .iter()
        .filter_map(|city| {
            let score = calculate_match_score(&query_lower, &city.name.to_lowercase());
            if score > 0.3 { // Minimum relevance threshold
                Some((city.clone(), score))
            } else {
                None
            }
        })
        .collect();
    
    // Sort by score (higher is better)
    scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    scored_results
}

/// Calculate a fuzzy match score between query and target string
fn calculate_match_score(query: &str, target: &str) -> f32 {
    if target.contains(query) {
        // Exact substring match gets high score
        1.0 - (target.len() - query.len()) as f32 / target.len() as f32
    } else {
        // Character-based fuzzy matching
        let query_chars: Vec<char> = query.chars().collect();
        let target_chars: Vec<char> = target.chars().collect();
        
        let mut matches = 0;
        let mut query_index = 0;
        
        for target_char in target_chars.iter() {
            if query_index < query_chars.len() && *target_char == query_chars[query_index] {
                matches += 1;
                query_index += 1;
            }
        }
        
        if matches == 0 {
            0.0
        } else {
            matches as f32 / query_chars.len() as f32 * 0.7 // Partial match penalty
        }
    }
}
