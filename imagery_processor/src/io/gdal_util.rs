#[cfg(feature = "gdal-io")]
use gdal::{Dataset, DriverManager};

#[cfg(feature = "gdal-io")]
pub fn write_u8_geotiff_basic(path: &str, data: &[u8], width: usize, height: usize) -> Result<(), gdal::errors::GdalError> {
    let drv = DriverManager::get_driver_by_name("GTiff")?;
    let mut ds = drv.create_with_band_type::<u8>(path, width as isize, height as isize, 1)?;
    let mut band = ds.rasterband(1)?;
    band.write((0,0), (width, height), data)?;
    Ok(())
}

#[cfg(feature = "gdal-io")]
pub fn copy_geo_from(src_path: &str, dst_path: &str) -> Result<(), gdal::errors::GdalError> {
    let src = Dataset::open(src_path)?;
    let mut dst = Dataset::open_ex(dst_path, gdal::DatasetOptions { ..Default::default() })?;
    if let Ok(gt) = src.geo_transform() {
        let _ = dst.set_geo_transform(&gt);
    }
    if let Ok(sref) = src.spatial_ref() {
        let wkt = sref.to_wkt()?;
        let _ = dst.set_projection(&wkt);
    }
    Ok(())
}

#[cfg(feature = "gdal-io")]
pub fn read_first_band_as_f32(path: &str) -> Result<(usize, usize, Vec<f32>, Option<f64>), gdal::errors::GdalError> {
    let ds = Dataset::open(path)?;
    let rb = ds.rasterband(1)?;
    let (w, h) = (rb.x_size() as usize, rb.y_size() as usize);
    let mut buf = vec![0.0f32; w * h];
    rb.read_as((0,0), (w, h), &mut buf, (w, h))?;
    let nodata = rb.no_data_value();
    Ok((w, h, buf, nodata))
}

#[cfg(feature = "gdal-io")]
pub fn read_first_band_as_f32_resampled(path: &str, out_w: usize, out_h: usize) -> Result<(usize, usize, Vec<f32>, Option<f64>), gdal::errors::GdalError> {
    let ds = Dataset::open(path)?;
    let rb = ds.rasterband(1)?;
    let (w, h) = (rb.x_size() as usize, rb.y_size() as usize);
    let mut buf = vec![0.0f32; out_w * out_h];
    rb.read_as((0,0), (w, h), &mut buf, (out_w, out_h))?; // resample to requested size
    let nodata = rb.no_data_value();
    Ok((out_w, out_h, buf, nodata))
}

#[cfg(feature = "gdal-io")]
pub fn write_f32_geotiff(path: &str, data: &[f32], width: usize, height: usize, nodata: Option<f64>) -> Result<(), gdal::errors::GdalError> {
    let drv = DriverManager::get_driver_by_name("GTiff")?;
    let mut ds = drv.create_with_band_type::<f32>(path, width as isize, height as isize, 1)?;
    {
        let mut band = ds.rasterband(1)?;
        band.write((0,0), (width, height), data)?;
        if let Some(nd) = nodata { let _ = band.set_no_data_value(nd); }
    }
    Ok(())
}

#[cfg(feature = "gdal-io")]
pub fn write_f32_cog(path: &str, data: &[f32], width: usize, height: usize, nodata: Option<f64>) -> Result<(), gdal::errors::GdalError> {
    // Try COG driver first; fall back to GTiff if unavailable
    if let Ok(drv) = DriverManager::get_driver_by_name("COG") {
        let mut ds = drv.create_with_band_type::<f32>(path, width as isize, height as isize, 1)?;
        {
            let mut band = ds.rasterband(1)?;
            band.write((0,0), (width, height), data)?;
            if let Some(nd) = nodata { let _ = band.set_no_data_value(nd); }
        }
        return Ok(());
    }
    // Fallback
    write_f32_geotiff(path, data, width, height, nodata)
}
