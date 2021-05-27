/// adapted from https://github.com/benbacardi/tyler/
/// original python code as comments

fn num_tiles(z: u8) -> u32 {
    // return math.pow(2,z)
    2_u32.pow(z as u32)
}

fn sec(x: f64) -> f64 {
    // return 1 / math.cos(x)
    x.cos().recip()
}

fn latlon2relative_xy(lat: f64,lon: f64) -> (f64, f64) {
    // x = (lon + 180) / 360
    let x = (lon + 180.0) / 360.0;
    // y = (1 - math.log(math.tan(math.radians(lat)) + sec(math.radians(lat))) / math.pi) / 2
    let y = (1.0 - (lat.to_radians().tan() + sec(lat.to_radians())).log(std::f64::consts::E) / std::f64::consts::PI) / 2.0;
    // return x, y
    (x, y)
}

fn latlon2xy(lat: f64, lon: f64, z: u8) -> (f64, f64) {
    // n = numTiles(z)
    let n = num_tiles(z) as f64;
    // x,y = latlon2relativeXY(lat, lon)
    let (x, y) = latlon2relative_xy(lat, lon);
    // return n*x, n*y
    (n * x, n * y)
}

fn tile_xy(lat: f64, lon: f64, z: u8) -> (i64, i64) {
    // x,y = latlon2xy(lat, lon, z)
    let (x, y) = latlon2xy(lat, lon, z);
    // return int(x), int(y)
    (x.round() as i64, y.round() as i64)
}

pub struct Map<'a> {
    tile_url: &'a str,
    tile_width: u32,
    tile_height: u32,
    zoom: u8,
    width: u32,
    height: u32,
    lat: f64,
    lon: f64,
}

impl<'a> Map<'a> {
    pub fn new(tile_url: &'a str, zoom: u8, width: u32, height: u32, lat: f64, lon: f64) -> Self {
        Map {
            tile_url,
            tile_width: 256,
            tile_height: 256,
            zoom,
            width,
            height,
            lat: (lat * 1000.0).round() / 1000.0,
            lon: (lon * 1000.0).round() / 1000.0,
        }
    }

    pub async fn get_map(&self, marker: Option<image::DynamicImage>) -> Result<image::DynamicImage, ()> {
        // tiles_x = int(math.ceil(self.width / self.tile_width)) + 2
        let tiles_x = (self.width / self.tile_width) + 2;
        // tiles_y = int(math.ceil(self.height / self.tile_height)) + 2
        let tiles_y = (self.height / self.tile_height) + 2;

        // x_row = range(-int(math.floor(tiles_x/2)),int(math.ceil(tiles_x/2)))
        let x_row = -((tiles_x / 2) as i64)..=((tiles_x / 2) as i64);
        // y_row = range(-int(math.floor(tiles_y/2)),int(math.ceil(tiles_y/2)))
        let y_row = -((tiles_y / 2) as i64)..=((tiles_y / 2) as i64);

        // x_offset, y_offset = tileXY(self.lat, self.lon, self.zoom)
        let (x_offset, y_offset) = tile_xy(self.lat, self.lon, self.zoom);
        // x_absolute, y_absolute = latlon2xy(self.lat, self.lon, self.zoom)
        let (x_absolute, y_absolute) = latlon2xy(self.lat, self.lon, self.zoom);

        // lat_center_diff = int((x_absolute - x_offset) * self.tile_width)
        let lat_center_diff = ((x_absolute - (x_offset as f64)) * (self.tile_width as f64)).round() as i32;
        // lon_center_diff = int((y_absolute - y_offset) * self.tile_height)
        let lon_center_diff = ((y_absolute - (y_offset as f64)) * (self.tile_height as f64)).round() as i32;

        // tiles = [[(x_offset + x, y_offset + y) for x in x_row] for y in y_row]
        let mut tiles = Vec::new();
        for y in y_row.clone() {
            let mut row = Vec::new();
            for x in x_row.clone() {
                row.push((x_offset + x, y_offset + y));
            }
            tiles.push(row);
        }

        // x_left = x_row.index(0) * self.tile_width + lat_center_diff
        let zero_index = x_row.enumerate()
            .find(|(_, x)| x == &0)
            .map(|(i, _)| i as i32)
            .ok_or_else(|| log::error!("0 index not found"))?;
        let x_left = (zero_index * (self.tile_width as i32) + lat_center_diff) as u32;
        // y_top = y_row.index(0) * self.tile_height + lon_center_diff
        let zero_index = y_row.enumerate()
            .find(|(_, x)| x == &0)
            .map(|(i, _)| i as i32)
            .ok_or_else(|| log::error!("0 index not found"))?;
        let y_top = (zero_index * (self.tile_height as i32) + lon_center_diff) as u32;

        // image_width = tiles_x * self.tile_width
        let image_width = tiles_x * self.tile_width;
        // image_height = tiles_y * self.tile_height
        let image_height = tiles_y * self.tile_height;

        // image = Image.new('RGBA', (image_width, image_height), (0,0,0,0))
        let mut image = image::DynamicImage::ImageRgba8(image::RgbaImage::new(image_width, image_height));
        // blank_image = Image.new('RGBA', (image_width, image_height), (0,0,0,0))
        let blank_image = image::DynamicImage::ImageRgba8(image::RgbaImage::new(image_width, image_height));

        // for row_offset, row in enumerate(tiles):
        for (row_offset, row) in tiles.into_iter().enumerate() {
            // for col_offset, (x, y) in enumerate(row):
            for (col_offset, (x, y)) in row.into_iter().enumerate() {
                // try:
                //     new_image = Image.open(self.get_tile(self.zoom, x, y))
                // except requests.HTTPError:
                //     new_image = blank_image
                let temp = self.get_tile(x, y).await;
                let new_image = match temp {
                    Ok(ref image) => image,
                    Err(_) => &blank_image,
                };
                // image.paste(new_image, ((col_offset * self.tile_width, row_offset * self.tile_height)))
                image::imageops::replace(&mut image, new_image, (col_offset as u32) * self.tile_width, (row_offset as u32) * self.tile_height);
            }
        }

        if let Some(mark) = marker {
            use image::GenericImageView;
            let (width, height) = mark.dimensions();
            image::imageops::overlay(&mut image, &mark, x_left - (width / 2), y_top - height);
        }

        // image = image.crop((
        //     int(x_left - (self.width / 2)),
        //     int(y_top - (self.height / 2)),
        //     int(x_left + (self.width / 2)),
        //     int(y_top + (self.height / 2)),
        // ))
        Ok(image.crop(
            x_left.saturating_sub(self.width / 2),
            y_top.saturating_sub(self.height / 2),
            self.width,
            self.height
        ))
    }

    async fn get_tile(&self, x: i64, y: i64) -> Result<image::DynamicImage, ()> {
        let tile_url = self.tile_url.replace("{s}", {
                use rand::Rng;
                match rand::thread_rng().gen_range(0..3) {
                    0 => "a",
                    1 => "b",
                    _ => "c",
                }
            })
            .replace("{z}", &self.zoom.to_string())
            .replace("{x}", &x.to_string())
            .replace("{y}", &y.to_string());
        let url = reqwest::Url::parse(&tile_url).map_err(|e| log::error!("error building tile url: {}", e))?;

        let res = reqwest::get(url)
            .await
            .map_err(|e| log::error!("error retrieving tile {}: {}", tile_url, e))?;

        if !res.status().is_success() {
            log::error!("tile {} retriever failed with status code {}", tile_url, res.status());
            Err(())
        }
        else {
            let bytes = res.bytes()
                .await
                .map_err(|e| log::error!("error reading tile {}: {}", tile_url, e))?;

            image::load_from_memory(&bytes)
                .map_err(|e| log::error!("error loading tile {}: {}", tile_url, e))
        }
    }
}
