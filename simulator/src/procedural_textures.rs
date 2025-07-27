use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::render_asset::RenderAssetUsages;

pub fn create_placeholder_earth_textures(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
) {
    info!("Creating placeholder Earth textures...");
    
    // Create a simple daymap texture (blue ocean, green land)
    let daymap = create_earth_daymap(&mut images);
    
    // Create a simple normal map (flat for now)
    let normalmap = create_normal_map(&mut images);
    
    // Create a specular map (white for water, black for land)
    let specular = create_specular_map(&mut images);
    
    // Create a night map (yellow dots for cities)
    let nightmap = create_night_map(&mut images);
    
    let earth_textures = crate::earth_textures::EarthTextures {
        daymap: Some(daymap),
        normalmap: Some(normalmap),
        specular: Some(specular),
        nightmap: Some(nightmap),
        clouds: None, // Skip clouds for now
        loading_complete: true, // Mark as complete since they're procedural
    };
    
    commands.insert_resource(earth_textures);
    info!("Placeholder Earth textures created!");
}

fn create_earth_daymap(images: &mut ResMut<Assets<Image>>) -> Handle<Image> {
    let width = 512;
    let height = 256;
    let mut data = Vec::with_capacity(width * height * 4);
    
    for y in 0..height {
        for x in 0..width {
            // Simple Earth-like pattern
            let lat = (y as f32 / height as f32 - 0.5) * std::f32::consts::PI;
            let lon = (x as f32 / width as f32) * 2.0 * std::f32::consts::PI;
            
            // Create continents with simple noise
            let continent = (lat.sin() * 3.0 + lon.cos() * 2.0 + 
                           (lat * 3.0).sin() * 0.5 + (lon * 4.0).cos() * 0.3).sin();
            
            if continent > 0.1 {
                // Land - green/brown
                data.push((50.0 + continent * 100.0) as u8);  // R
                data.push((100.0 + continent * 80.0) as u8);  // G  
                data.push((30.0 + continent * 50.0) as u8);   // B
            } else {
                // Ocean - blue
                data.push((20.0 - continent * 50.0) as u8);   // R
                data.push((50.0 - continent * 100.0) as u8);  // G
                data.push((150.0 - continent * 100.0) as u8); // B
            }
            data.push(255); // A
        }
    }
    
    let image = Image::new(
        Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    
    images.add(image)
}

fn create_normal_map(images: &mut ResMut<Assets<Image>>) -> Handle<Image> {
    let width = 512;
    let height = 256;
    let mut data = Vec::with_capacity(width * height * 4);
    
    // Simple flat normal map (pointing up)
    for _y in 0..height {
        for _x in 0..width {
            data.push(128); // X = 0.5 (neutral)
            data.push(128); // Y = 0.5 (neutral)  
            data.push(255); // Z = 1.0 (pointing up)
            data.push(255); // A
        }
    }
    
    let image = Image::new(
        Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    
    images.add(image)
}

fn create_specular_map(images: &mut ResMut<Assets<Image>>) -> Handle<Image> {
    let width = 512;
    let height = 256;
    let mut data = Vec::with_capacity(width * height * 4);
    
    for y in 0..height {
        for x in 0..width {
            // Same logic as daymap to determine water vs land
            let lat = (y as f32 / height as f32 - 0.5) * std::f32::consts::PI;
            let lon = (x as f32 / width as f32) * 2.0 * std::f32::consts::PI;
            
            let continent = (lat.sin() * 3.0 + lon.cos() * 2.0 + 
                           (lat * 3.0).sin() * 0.5 + (lon * 4.0).cos() * 0.3).sin();
            
            if continent > 0.1 {
                // Land - black (non-metallic)
                data.push(0);   // Metallic
                data.push(200); // Roughness (rough land)
                data.push(0);   // Unused
            } else {
                // Water - white metallic, smooth
                data.push(255); // Metallic (reflective water)
                data.push(10);  // Roughness (smooth water)
                data.push(0);   // Unused
            }
            data.push(255); // A
        }
    }
    
    let image = Image::new(
        Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    
    images.add(image)
}

fn create_night_map(images: &mut ResMut<Assets<Image>>) -> Handle<Image> {
    let width = 512;
    let height = 256;
    let mut data = Vec::with_capacity(width * height * 4);
    
    for y in 0..height {
        for x in 0..width {
            // Create city lights in populated areas
            let lat = (y as f32 / height as f32 - 0.5) * std::f32::consts::PI;
            let lon = (x as f32 / width as f32) * 2.0 * std::f32::consts::PI;
            
            // Check if we're on land
            let continent = (lat.sin() * 3.0 + lon.cos() * 2.0 + 
                           (lat * 3.0).sin() * 0.5 + (lon * 4.0).cos() * 0.3).sin();
            
            if continent > 0.1 {
                // On land - add some city lights
                let city_noise = ((lat * 10.0).sin() * (lon * 8.0).cos() + 
                                (lat * 15.0).cos() * (lon * 12.0).sin()).abs();
                
                if city_noise > 0.8 {
                    // City light - warm yellow
                    data.push(100); // R
                    data.push(80);  // G
                    data.push(20);  // B
                } else {
                    // Dark land
                    data.push(0);   // R
                    data.push(0);   // G
                    data.push(0);   // B
                }
            } else {
                // Ocean - dark
                data.push(0);   // R
                data.push(0);   // G
                data.push(0);   // B
            }
            data.push(255); // A
        }
    }
    
    let image = Image::new(
        Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    
    images.add(image)
}
