// use std::fs::{File, OpenOptions};
// use std::io::{Read, Write};
use std::path::Path;
// use std::time::{Instant, Duration};
use std::time::Duration;

// use tokio::timer::Delay;
use tokio::future::FutureExt;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use mysql_async::{prelude::Queryable, Conn};

use futures_util::try_stream::TryStreamExt;

use hyper::Client;
use hyper_tls::HttpsConnector;

use chrono::{Local, DateTime, Timelike};
use chrono::offset::TimeZone;

use serde_json::{json, value::Value};

use async_trait::async_trait;

use log::error;

use super::BotConfigs;

use crate::entities::{Pokemon, Raid, Pokestop, Gender, Weather, Quest};
use crate::lists::{LIST, MOVES, FORMS, GRUNTS};
use crate::config::CONFIG;
use crate::db::MYSQL;
use crate::telegram::{send_photo, CallResult, Image};

fn truncate_str(s: &str, limit: usize, placeholder: char) -> String {
    if s.is_empty() {
        return placeholder.to_string();
    }

    let mut chars: Vec<char> = s.chars().take(limit + 1).collect();
    if chars.len() > limit {
        chars.truncate(limit - 1);
        chars.push('.');
        chars.push('.');
    }

    chars.into_iter().collect()
}

async fn open_font(path: &str) -> Result<rusttype::Font<'static>, ()> {
    let mut file = File::open(path).await.map_err(|e| error!("error opening font {}: {}", path, e))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).await.map_err(|e| error!("error reading font {}: {}", path, e))?;
    rusttype::Font::from_bytes(data).map_err(|e| error!("error decoding font {}: {}", path, e))
}

async fn open_image(path: &str) -> Result<image::DynamicImage, ()> {
    let mut file = File::open(path).await.map_err(|e| error!("error opening image {}: {}", path, e))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).await.map_err(|e| error!("error reading image {}: {}", path, e))?;
    image::load_from_memory_with_format(&data, image::ImageFormat::PNG).map_err(|e| error!("error opening image {}: {}", path, e))
}

async fn save_image(img: image::DynamicImage, path: &str) -> Result<Vec<u8>, ()> {
    let mut out = Vec::new();
    img.write_to(&mut out, image::ImageOutputFormat::PNG).map_err(|e| error!("error converting image {}: {}", path, e))?;

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)
        .await
        .map_err(|e| error!("error saving image {}: {}", path, e))?;
    file.write_all(&out).await.map_err(|e| error!("error writing image {}: {}", path, e))?;

    Ok(out)
}

fn get_text_width(font: &rusttype::Font, scale: rusttype::Scale, text: &str) -> i32 {
    let space = font.glyph(' ').scaled(scale).h_metrics().advance_width.round() as i32;
    font.layout(text, scale, rusttype::Point { x: 0f32, y: 0f32 })
        .fold(0, |acc, l| acc + l.pixel_bounding_box().map(|bb| bb.width()).unwrap_or_else(|| space))
}

fn meteo_icon(meteo: u8) -> Result<String, ()> {
    Ok(format!(" {}", String::from_utf8(match meteo {
        1 => vec![0xe2, 0x98, 0x80, 0xef, 0xb8, 0x8f],//CLEAR
        2 => vec![0xf0, 0x9f, 0x8c, 0xa7],//RAINY
        3 => vec![0xe2, 0x9b, 0x85, 0xef, 0xb8, 0x8f],//PARTLY_CLOUDY
        4 => vec![0xe2, 0x98, 0x81, 0xef, 0xb8, 0x8f],//OVERCAST
        5 => vec![0xf0, 0x9f, 0x8c, 0xac],//WINDY
        6 => vec![0xe2, 0x9d, 0x84, 0xef, 0xb8, 0x8f],//SNOW
        7 => vec![0xf0, 0x9f, 0x8c, 0xab],//FOG
        _ => return Ok(String::new()),
    }).map_err(|e| error!("error converting meteo icon: {}", e))?))
}

#[async_trait]
pub trait Message {
    async fn send(&self, chat_id: &str, image: Image, map_type: &str) -> Result<(), ()> {
        match send_photo(&CONFIG.telegram.bot_token, chat_id, image, Some(&self.get_caption().await?), None, None, None, Some(self.message_button(chat_id, map_type)?)).await {
            Ok(_) => {
                let conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
                let query = format!("UPDATE utenti_config_bot SET sent = sent + 1 WHERE user_id = {}", chat_id);
                self.update_stats(conn).await?.query(query).await.map_err(|e| error!("MySQL query error: {}", e))?;
                Ok(())
            },
            Err(CallResult::Body((_, body))) => {
                let json: Value = serde_json::from_str(&body).map_err(|e| error!("error while decoding {}: {}", body, e))?;

                // blocked, disable bot
                if json["description"] == "Forbidden: bot was blocked by the user" {
                    let conn = MYSQL.get_conn().await.map_err(|e| error!("MySQL retrieve connection error: {}", e))?;
                    let query = format!("UPDATE utenti_config_bot SET enabled = 0 WHERE user_id = {}", chat_id);
                    conn.query(query).await.map_err(|e| error!("MySQL query error: {}", e))?;
                    // apply
                    BotConfigs::reload(vec![chat_id.to_owned()]).await
                }
                else {
                    Err(())
                }
            },
            _ => Err(()),
        }
    }

    async fn get_map(&self) -> Result<image::DynamicImage, ()> {
        // $lat = number_format(round($ilat, 3), 3);
        // $lon = number_format(round($ilon, 3), 3);
        // $map_path = "../../data/bot/img_maps/" . $lat . "_" . $lon . ".png";
        let map_path_str = format!("{}img_maps/{:.3}_{:.3}.png", CONFIG.images.bot, self.get_latitude(), self.get_longitude());

        let map_path = Path::new(&map_path_str);
        if map_path.exists() {
            return open_image(&map_path_str).await;
        }

        let m_link = format!("https://maps.googleapis.com/maps/api/staticmap?center={:.3},{:.3}&zoom=14&size=280x101&maptype=roadmap&markers={:.3},{:.3}&key={}", self.get_latitude(), self.get_longitude(), self.get_latitude(), self.get_longitude(), CONFIG.google.maps_key)
            .parse()
            .map_err(|e| error!("Error building Google URI: {}", e))?;
        let https = HttpsConnector::new().unwrap();
        let future = Client::builder().build::<_, hyper::Body>(https).get(m_link);
        let res = match CONFIG.google.timeout {
                Some(timeout) => future.timeout(Duration::from_secs(timeout)).await.map_err(|e| error!("timeout calling google maps: {}", e))?,
                None => future.await,
            }.map_err(|e| error!("error calling google maps: {}", e))?;

        let chunks = res.into_body().try_concat().await.map_err(|e| error!("error reading google maps response: {}", e))?;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&map_path_str)
            .await
            .map_err(|e| error!("error creating file {}: {}", map_path_str, e))?;
        let buf = chunks.to_vec();
        file.write_all(&buf).await.map_err(|e| error!("error creating file {}: {}", map_path_str, e))?;
        
        image::load_from_memory_with_format(&buf, image::ImageFormat::PNG)
            .map_err(|e| error!("error opening map image {}: {}", map_path_str, e))
    }

    async fn prepare(&self, now: DateTime<Local>) -> Result<Image, ()> {
        let map = self.get_map().await?;
        let bytes = self.get_image(map).await?;

        if let Some(chat_id) = CONFIG.telegram.cache_chat.as_ref() {
            let mut retries: u8 = 0;
            loop {
                let temp = format!("{}\n{} retries", now.format("%F %T").to_string(), retries);
                match send_photo(&CONFIG.telegram.bot_token, chat_id, Image::Bytes(bytes.clone()), Some(&temp), None, None, None, None).await {
                    Ok(body) => {
                        let json: Value = serde_json::from_str(&body).map_err(|e| error!("error while decoding Telegram response: {}\n{}", e, body))?;

                        if let Some(sizes) = json["result"]["photo"].as_array() {
                            // scan various formats to select the best one
                            let mut best_index = 0;
                            for i in 1..sizes.len() {
                                if sizes[best_index]["file_size"].as_u64() < sizes[i]["file_size"].as_u64() {
                                    best_index = i;
                                }
                            }

                            return Ok(Image::FileId(sizes[best_index]["file_id"].as_str().map(|s| s.to_owned()).ok_or_else(|| ())?));
                        }
                        else {
                            error!("error while reading Telegram response: photos isn't an array\n{}", body);
                            return Err(());
                        }
                    },
                    Err(CallResult::Body((status, body))) => {
                        if status == 429u16 || status == 504u16 {
                            retries += 1;
                            continue;
                        }
                        else {
                            error!("error while reading Telegram response: {}", body);
                            return Err(());
                        }
                    },
                    _ => {
                        return Err(());
                    },
                }
            }
        }
        else {
            Ok(Image::Bytes(bytes))
        }
    }

    fn message_button(&self, _chat_id: &str, mtype: &str) -> Result<Value, ()> {
        let lat = self.get_latitude();
        let lon = self.get_longitude();

        let maplink = match mtype {
            "g" => format!("https://maps.google.it/maps/?q={},{}", lat, lon),
            "g2" => format!("https://www.google.it/maps/place/{},{}", lat, lon),
            "g3" => format!("https://www.google.com/maps/search/?api=1&query={},{}", lat, lon),
            "gd" => format!("https://www.google.com/maps/dir/?api=1&destination={},{}", lat, lon),
            "a" => format!("http://maps.apple.com/?ll={},{}", lat, lon),
            "w" => format!("https://waze.com/ul?ll={},{}", lat, lon),
            _ => format!("https://maps.google.it/maps/?q={},{}", lat, lon),
        };
        let title = format!("{} Mappa", String::from_utf8(vec![0xf0, 0x9f, 0x8c, 0x8e]).map_err(|e| error!("error encoding map icon: {}", e))?);

        Ok(json!({
            "inline_keyboard": [[{
                "text": title,
                "url": maplink
            }]]
        }))
    }

    fn get_latitude(&self) -> f64;

    fn get_longitude(&self) -> f64;

    async fn get_caption(&self) -> Result<String, ()>;

    async fn get_image(&self, map: image::DynamicImage) -> Result<Vec<u8>, ()>;

    async fn update_stats(&self, conn: Conn) -> Result<Conn, ()>;
}

#[derive(Debug)]
pub struct PokemonMessage {
    pub pokemon: Box<Pokemon>,
    pub iv: Option<f32>,
    pub distance: f64,
    pub direction: String,
    pub debug: Option<String>,
}

#[async_trait]
impl Message for PokemonMessage {
    fn get_latitude(&self) -> f64 {
        self.pokemon.latitude
    }

    fn get_longitude(&self) -> f64 {
        self.pokemon.longitude
    }

    async fn get_caption(&self) -> Result<String, ()> {
        // $icon_pkmn = "\xf0\x9f\x94\xb0 #" . $t_msg["pokemon_id"];
        // $icon_raid = "\xe2\x9a\x94\xef\xb8\x8f";
        // if (intval(date("Ymd")) >= 20171222 && intval(date("Ymd")) <= 20180106) {
        //   $icon_pkmn = "\xf0\x9f\x8e\x81"; // natale
        //   $icon_raid = "\xf0\x9f\x8e\x84"; // natale
        // }
        let date = Local::today();
        let date: usize = date.format("%m%d").to_string().parse().map_err(|e| error!("error parsing date: {}", e))?;
        let icon = if date < 106 || date > 1222 {
            String::from_utf8(vec![0xf0, 0x9f, 0x8e, 0x81])//natale
                .map_err(|e| error!("error parsing pokemon christmas icon: {}", e))?
        }
        else {
            format!("{} #{}",
                String::from_utf8(vec![0xf0, 0x9f, 0x94, 0xb0])
                    .map_err(|e| error!("error parsing pokemon icon: {}", e))?,
                self.pokemon.pokemon_id
            )
        };

        // $dir_icon = " " . $t_msg["direction"];
        // if ($t_msg["distance"] == 0) {
        //     $dir_icon = " \xf0\x9f\x8f\xa0";
        // }
        let dir_icon = if self.distance > 0f64 {
            self.direction.clone()
        }
        else {
            String::from_utf8(vec![0xf0, 0x9f, 0x8f, 0xa0]).map_err(|e| error!("error parsing direction icon: {}", e))?
        };

        // if ($t_msg["cp"] != "") {
        let caption = if let Some(iv) = self.iv {
            // $v_iv = GetIV($t_msg["atk_iv"], $t_msg["def_iv"], $t_msg["sta_iv"]);

            // $t_corpo = $icon_pkmn . " " . strtoupper($PKMNS[$t_msg["pokemon_id"]]["name"]);
            // $t_corpo .= ($t_msg["pokemon_id"] == 201 ? " (" . $unown_letter[$t_msg["form"]] . ")" : "");
            // $t_corpo .= " (" . $v_iv . "%)" . MeteoIcon($t_msg["wb"]) . "\n";
            // $t_corpo .= "PL " . number_format($t_msg["cp"], 0, ",", ".") . " | Lv " . $t_msg["level"] . "\n";
            // $t_corpo .= $t_msg["distance"] . "km" . $dir_icon . " | " . date("H:i", $t_msg["expire_timestamp"]);
            let gender = self.pokemon.gender.get_glyph();
            format!("{} {}{}{} ({:.0}%){}\n{}{:.1} km {} | {}",
                icon,
                LIST.read().await.get(&self.pokemon.pokemon_id).map(|p| p.name.to_uppercase()).unwrap_or_else(String::new),
                gender,
                match self.pokemon.form {
                    Some(id) => FORMS.read().await.get(&id).map(|s| format!(" ({})", s)),
                    None => None,
                }.unwrap_or_else(String::new),
                iv.round(),
                self.pokemon.weather.and_then(|id| meteo_icon(id).ok()).unwrap_or_else(String::new),
                match (self.pokemon.cp, self.pokemon.pokemon_level) {
                    (Some(cp), Some(level)) => format!("PL {} | Lv {}\n", cp, level),
                    _ => String::new(),
                },
                self.distance,
                dir_icon,
                Local.timestamp(self.pokemon.disappear_time, 0).format("%T").to_string()
            ).replace(&gender.repeat(2), &gender)//fix nidoran double gender
        }
        else {
            // $t_corpo = $icon_pkmn . " " . strtoupper($PKMNS[$t_msg["pokemon_id"]]["name"]);
            // $t_corpo .= ($t_msg["pokemon_id"] == 201 ? " (" . $unown_letter[$t_msg["form"]] . ")" : "") . MeteoIcon($t_msg["wb"]) . "\n";
            // $t_corpo .= $t_msg["distance"] . "km" . $dir_icon . " | " . date("H:i", $t_msg["expire_timestamp"]);
            let gender = self.pokemon.gender.get_glyph();
            format!("{} {}{}{}{}\n{:.1} km {} | {}",
                icon,
                LIST.read().await.get(&self.pokemon.pokemon_id).map(|p| p.name.to_uppercase()).unwrap_or_else(String::new),
                gender,
                match self.pokemon.form {
                    Some(id) => FORMS.read().await.get(&id).map(|s| format!(" ({})", s)),
                    None => None,
                }.unwrap_or_else(String::new),
                self.pokemon.weather.and_then(|id| meteo_icon(id).ok()).unwrap_or_else(String::new),
                self.distance,
                dir_icon,
                Local.timestamp(self.pokemon.disappear_time, 0).format("%T").to_string()
            ).replace(&gender.repeat(2), &gender)//fix nidoran double gender
        };

        Ok(match self.debug {
            Some(ref s) => format!("{}\n\n{}", caption, s),
            None => caption,
        })
    }

    async fn get_image(&self, map: image::DynamicImage) -> Result<Vec<u8>, ()> {
        let now = Local::now();
        let img_path_str = format!("{}img_sent/poke_{}_{}_{}.png", CONFIG.images.bot, now.format("%Y%m%d%H").to_string(), self.pokemon.encounter_id, self.iv.map(|iv| format!("{:.0}", iv)).unwrap_or_else(String::new));

        let img_path = Path::new(&img_path_str);
        if img_path.exists() {
            let mut image = File::open(&img_path).await.map_err(|e| error!("error opening pokemon image {}: {}", img_path_str, e))?;
            let mut bytes = Vec::new();
            image.read_to_end(&mut bytes).await.map_err(|e| error!("error reading pokemon image {}: {}", img_path_str, e))?;
            return Ok(bytes);
        }

        let f_cal1 = {
            let font = format!("{}fonts/calibri.ttf", CONFIG.images.sender);
            open_font(&font).await?
        };
        let f_cal2 = {
            let font = format!("{}fonts/calibrib.ttf", CONFIG.images.sender);
            open_font(&font).await?
        };
        let scale11 = rusttype::Scale::uniform(16f32);
        let scale12 = rusttype::Scale::uniform(17f32);
        let scale13 = rusttype::Scale::uniform(18f32);
        let scale18 = rusttype::Scale::uniform(23f32);

        // $mBg = null;
        let mut background = {
            let path = format!("{}{}", CONFIG.images.sender, match self.iv {
                Some(i) if i < 80f32 => "images/msg-bgs/msg-poke-big-norm.png",
                Some(i) if i >= 80f32 && i < 90f32 => "images/msg-bgs/msg-poke-big-med.png",
                Some(i) if i >= 90f32 && i < 100f32 => "images/msg-bgs/msg-poke-big-hi.png",
                Some(i) if i >= 100f32 => "images/msg-bgs/msg-poke-big-top.png",
                _ => "images/msg-bgs/msg-poke-sm.png",
            });
            open_image(&path).await?
        };

        let pokemon = match self.pokemon.form {
            Some(form) if form > 0 => {
                let image = format!("{}img/pkmns/shuffle/{}-{}.png",
                    CONFIG.images.assets,
                    self.pokemon.pokemon_id,
                    form
                );
                match open_image(&image).await {
                    Ok(img) => img,
                    Err(_) => {
                        let image = format!("{}img/pkmns/shuffle/{}.png",
                            CONFIG.images.assets,
                            self.pokemon.pokemon_id
                        );
                        open_image(&image).await?
                    },
                }
            },
            _ => {
                let image = format!("{}img/pkmns/shuffle/{}.png",
                    CONFIG.images.assets,
                    self.pokemon.pokemon_id
                );
                open_image(&image).await?
            },
        };

        image::imageops::overlay(&mut background, &pokemon, 5, 5);

        match self.pokemon.gender {
            Gender::Male | Gender::Female => {
                let path = format!("{}img/{}.png", CONFIG.images.assets, if self.pokemon.gender == Gender::Female { "female" } else { "male" });
                let icon = open_image(&path).await?;
                image::imageops::overlay(&mut background, &icon, 32, 32);
            }
            _ => {},
        }

        // imagettftext($mBg, 18, 0, 63, 25, 0x00000000, $f_cal2, strtoupper($p_name));
        let name = LIST.read().await.get(&self.pokemon.pokemon_id).map(|p| {
            // fix nidoran gender
            let gender = self.pokemon.gender.get_glyph();
            p.name.replace(&gender, "").to_uppercase()
        }).unwrap_or_else(String::new);
        imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 63, 7, scale18, &f_cal2, &name);

        if let Some(id) = self.pokemon.form {
            if let Some(form_name) = FORMS.read().await.get(&id) {
                let dm = get_text_width(&f_cal2, scale18, &name);
                imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 73 + dm as u32, 7, scale11, &f_cal2, &format!("({})", form_name));
            }
        }

        // imagettftext($mBg, 12, 0, 82, 46, 0x00000000, $f_cal2, $v_exit);
        let v_exit = Local.timestamp(self.pokemon.disappear_time, 0);
        imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 82, 34, scale12, &f_cal2, &v_exit.format("%T").to_string());

        //     imagecopymerge($mBg, $mMap, 0, ($v_ivs ? 136 : 58), 0, 0, 280, 101, 100);
        image::imageops::overlay(&mut background, &map, 0, if self.iv.is_some() { 136 } else { 58 });

        // //////////////////////////////////////////////
        // // IV, PL e MOSSE
        if let Some(iv) = self.iv {
            // $dm = imagettfbbox(11, 0, $f_cal1, strtoupper($m_move1));
            // imagettftext($mBg, 11, 0, 80 - (abs($dm[4] - $dm[6]) / 2), 75, 0x00000000, $f_cal1, strtoupper($m_move1));
            let m_move1 = match self.pokemon.move_1 {
                    Some(i) => MOVES.read().await.get(&i).map(|s| s.to_uppercase()),
                    None => None,
                }.unwrap_or_else(|| String::from("-"));
            let dm = get_text_width(&f_cal1, scale11, &m_move1);
            imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 80 - (dm / 2) as u32, 64, scale11, &f_cal1, &m_move1);
            // $dm = imagettfbbox(11, 0, $f_cal1, strtoupper($m_move2));
            // imagettftext($mBg, 11, 0, 200 - (abs($dm[4] - $dm[6]) / 2), 75, 0x00000000, $f_cal1, strtoupper($m_move2));
            let m_move2 = match self.pokemon.move_2 {
                    Some(i) => MOVES.read().await.get(&i).map(|s| s.to_uppercase()),
                    None => None,
                }.unwrap_or_else(|| String::from("-"));
            let dm = get_text_width(&f_cal1, scale11, &m_move2);
            imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 200 - (dm / 2) as u32, 64, scale11, &f_cal1, &m_move2);

            let v_ivcolor = match self.iv {
                Some(i) if i == 0f32 => image::Rgba::<u8>([0x2D, 0x90, 0xFF, 0]),//0x002D90FF, // NULL Azzurro
                Some(i) if i >= 80f32 && i < 90f32 => image::Rgba::<u8>([0xFF, 0x62, 0x14, 0]),//0x00FF6214, // MED Arancione
                Some(i) if i >= 90f32 && i < 100f32 => image::Rgba::<u8>([0xFF, 0, 0, 0]),//0x00FF0000, // HI Rosso
                Some(i) if i >= 100f32 => image::Rgba::<u8>([0xDC, 0, 0xEA, 0]),//0x00DC00EA, // TOP Viola
                _ => image::Rgba::<u8>([0, 0, 0, 0]),//0x00000000,
            };
            // $dm = imagettfbbox(13, 0, $f_cal2, "IV " . $v_iv . " %");
            // imagettftext($mBg, 13, 0, 80 - (abs($dm[4] - $dm[6]) / 2), 100, $v_ivcolor, $f_cal2, "IV " . $v_iv . " %");
            let text = format!("IV {:.0}%", iv.round());
            let dm = get_text_width(&f_cal2, scale13, &text);
            imageproc::drawing::draw_text_mut(&mut background, v_ivcolor, 80 - (dm / 2) as u32, 87, scale13, &f_cal2, &text);

            let v_plcolor = match self.pokemon.pokemon_level {
                Some(i) if i >= 25 && i < 30 => image::Rgba::<u8>([0xFF, 0x62, 0x14, 0]),//0x00FF6214, // MED Arancione
                Some(i) if i >= 30 && i < 35 => image::Rgba::<u8>([0xFF, 0, 0, 0]),//0x00FF0000, // HI Rosso
                Some(i) if i >= 35 => image::Rgba::<u8>([0xDC, 0, 0xEA, 0]),//0x00DC00EA, // TOP Viola
                _ => image::Rgba::<u8>([0, 0, 0, 0]),//0x00000000,
            };
            // $dm = imagettfbbox(13, 0, $f_cal2, "PL " . number_format($v_pl, 0, '', '.'));
            // imagettftext($mBg, 13, 0, 200 - (abs($dm[4] - $dm[6]) / 2), 100, $v_plcolor, $f_cal2, "PL " . number_format($v_pl, 0, '', '.'));
            let text = format!("PL {}", self.pokemon.cp.unwrap_or_else(|| 0));
            let dm = get_text_width(&f_cal2, scale13, &text);
            imageproc::drawing::draw_text_mut(&mut background, v_plcolor, 200 - (dm / 2) as u32, 87, scale13, &f_cal2, &text);

            // $v_str = "ATK: " . $v_atk . "   DEF: " . $v_def . "   STA: " . $v_sta;
            // $dm = imagettfbbox(12, 0, $f_cal1, $v_str);
            // imagettftext($mBg, 12, 0, 140 - (abs($dm[4] - $dm[6]) / 2), 123, 0x00000000, $f_cal1, $v_str);
            let text = format!("ATK: {}   DEF: {}   STA: {}", self.pokemon.individual_attack.unwrap_or_else(|| 0), self.pokemon.individual_defense.unwrap_or_else(|| 0), self.pokemon.individual_stamina.unwrap_or_else(|| 0));
            let dm = get_text_width(&f_cal1, scale12, &text);
            imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 140 - (dm / 2) as u32, 111, scale12, &f_cal1, &text);
        }

        save_image(background, &img_path_str).await
    }

    fn message_button(&self, chat_id: &str, mtype: &str) -> Result<Value, ()> {
        let lat = self.get_latitude();
        let lon = self.get_longitude();

        let maplink = match mtype {
            "g" => format!("https://maps.google.it/maps/?q={},{}", lat, lon),
            "g2" => format!("https://www.google.it/maps/place/{},{}", lat, lon),
            "g3" => format!("https://www.google.com/maps/search/?api=1&query={},{}", lat, lon),
            "gd" => format!("https://www.google.com/maps/dir/?api=1&destination={},{}", lat, lon),
            "a" => format!("http://maps.apple.com/?ll={},{}", lat, lon),
            "w" => format!("https://waze.com/ul?ll={},{}", lat, lon),
            _ => format!("https://maps.google.it/maps/?q={},{}", lat, lon),
        };
        let title = format!("{} Mappa", String::from_utf8(vec![0xf0, 0x9f, 0x8c, 0x8e]).map_err(|e| error!("error encoding map icon: {}", e))?);

        let mut keyboard = json!({
                "inline_keyboard": [[{
                    "text": title,
                    "url": maplink
                }]]
            });
if chat_id == "25900594" || chat_id == "112086777" || chat_id == "9862788" || chat_id == "82417031" {//DEBUG
        // watch button available only on crossing-hour spawns
        if Local::now().hour() != Local.timestamp(self.pokemon.disappear_time, 0).hour() {
            match (self.pokemon.individual_attack, self.pokemon.individual_defense, self.pokemon.individual_stamina, keyboard["inline_keyboard"].as_array_mut()) {
                (Some(_), Some(_), Some(_), Some(a)) => {
                    a.push(json!([{
                        "text": format!("{} Avvisami se cambia il Meteo", String::from_utf8(vec![0xE2, 0x9B, 0x85]).map_err(|e| error!("error encoding meteo icon: {}", e))?),
                        "callback_data": format!("watch|{}|{}|{}", lat, lon, self.pokemon.disappear_time)
                    }]));
                },
                _ => {},
            }
        }
}//DEBUG
        Ok(keyboard)
    }

    async fn update_stats(&self, conn: Conn) -> Result<Conn, ()> {
        let query = format!("INSERT INTO bot_sent_pkmn (pokemon_id, sent) VALUES ({}, 1) ON DUPLICATE KEY UPDATE sent = sent + 1", self.pokemon.pokemon_id);
        let res = conn.query(query).await.map_err(|e| error!("MySQL query error: {}", e))?;
        res.drop_result().await.map_err(|e| error!("MySQL drop result error: {}", e))
    }
}

#[derive(Debug)]
pub struct RaidMessage {
    pub raid: Raid,
    pub distance: f64,
    pub debug: Option<String>,
}

#[async_trait]
impl Message for RaidMessage {
    fn get_latitude(&self) -> f64 {
        self.raid.latitude
    }

    fn get_longitude(&self) -> f64 {
        self.raid.longitude
    }

    async fn get_caption(&self) -> Result<String, ()> {
        // $icon_pkmn = "\xf0\x9f\x94\xb0 #" . $t_msg["pokemon_id"];
        // $icon_raid = "\xe2\x9a\x94\xef\xb8\x8f";
        // if (intval(date("Ymd")) >= 20171222 && intval(date("Ymd")) <= 20180106) {
        //   $icon_pkmn = "\xf0\x9f\x8e\x81"; // natale
        //   $icon_raid = "\xf0\x9f\x8e\x84"; // natale
        // }
        let date = Local::today();
        let date: usize = date.format("%m%d").to_string().parse().map_err(|e| error!("error parsing date: {}", e))?;
        let icon = if date < 106 || date > 1222 {
            String::from_utf8(vec![0xf0, 0x9f, 0x8e, 0x84])//natale
                .map_err(|e| error!("error parsing raid christmas icon: {}", e))?
        }
        else {
            String::from_utf8(vec![0xe2, 0x9a, 0x94, 0xef, 0xb8, 0x8f])
                .map_err(|e| error!("error parsing raid icon: {}", e))?
        };

        let caption = if let Some(pokemon_id) = self.raid.pokemon_id.and_then(|id| if id > 0 { Some(id) } else { None }) {
            // $t_corpo = $icon_raid . " "; // Battaglia
            // $t_corpo .= "RAID " . strtoupper($PKMNS[$t_msg["pokemon_id"]]["name"]) . " iniziato\n";
            // $t_corpo .= "\xf0\x9f\x93\x8d " . (strlen($gym_name) > 36 ? substr($gym_name, 0, 35) . ".." : $gym_name) . "\n";
            // $t_corpo .= "\xf0\x9f\x95\x92 Termina: " . date("H:i:s", $t_msg["time_end"]);
            format!("{} RAID {}{} iniziato\n{} {}\n{} Termina: {}",//debug
                icon,
                LIST.read().await.get(&pokemon_id).map(|p| p.name.to_uppercase()).unwrap_or_else(String::new),
                match self.raid.form {
                    Some(id) => FORMS.read().await.get(&id).map(|s| format!(" ({})", s)),
                    None => None,
                }.unwrap_or_else(String::new),
                String::from_utf8(if self.raid.ex_raid_eligible { vec![0xE2, 0x9B, 0xB3] } else { vec![0xf0, 0x9f, 0x93, 0x8d] }).map_err(|e| error!("error parsing POI icon: {}", e))?,
                self.raid.gym_name,
                String::from_utf8(vec![0xf0, 0x9f, 0x95, 0x92]).map_err(|e| error!("error parsing clock icon: {}", e))?,
                Local.timestamp(self.raid.end, 0).format("%T").to_string()
            )
        }
        else {
            // $t_corpo = "\xf0\x9f\xa5\x9a "; // Uovo
            // $t_corpo .= "RAID liv. " . $t_msg["level"] . "\n";
            // $t_corpo .= "\xf0\x9f\x93\x8d " . (strlen($gym_name) > 36 ? substr($gym_name, 0, 35) . ".." : $gym_name) . "\n";
            // $t_corpo .= "\xf0\x9f\x95\x92 Schiude: " . date("H:i:s", $t_msg["time_battle"]);
            format!("{} RAID liv. {}\n{} {}\n{} Schiude: {}",//debug
                String::from_utf8(vec![0xf0, 0x9f, 0xa5, 0x9a]).map_err(|e| error!("error parsing egg icon: {}", e))?,
                self.raid.level,
                String::from_utf8(if self.raid.ex_raid_eligible { vec![0xE2, 0x9B, 0xB3] } else { vec![0xf0, 0x9f, 0x93, 0x8d] }).map_err(|e| error!("error parsing POI icon: {}", e))?,
                self.raid.gym_name,
                String::from_utf8(vec![0xf0, 0x9f, 0x95, 0x92]).map_err(|e| error!("error parsing clock icon: {}", e))?,
                Local.timestamp(self.raid.start, 0).format("%T").to_string()
            )
        };

        Ok(match self.debug {
            Some(ref s) => format!("{}\n\n{}", caption, s),
            None => caption,
        })
    }

    async fn get_image(&self, map: image::DynamicImage) -> Result<Vec<u8>, ()> {
        let now = Local::now();
        let img_path_str = format!("{}img_sent/raid_{}_{}_{}_{}.png", CONFIG.images.bot, now.format("%Y%m%d%H").to_string(), self.raid.gym_id, self.raid.start, self.raid.pokemon_id.map(|i| i.to_string()).unwrap_or_else(String::new));
        let img_path = Path::new(&img_path_str);

        if img_path.exists() {
            let mut image = File::open(&img_path).await.map_err(|e| error!("error opening raid image {}: {}", img_path_str, e))?;
            let mut bytes = Vec::new();
            image.read_to_end(&mut bytes).await.map_err(|e| error!("error reading raid image {}: {}", img_path_str, e))?;
            return Ok(bytes);
        }

        let f_cal1 = {
            let font = format!("{}fonts/calibri.ttf", CONFIG.images.sender);
            open_font(&font).await?
        };
        let f_cal2 = {
            let font = format!("{}fonts/calibrib.ttf", CONFIG.images.sender);
            open_font(&font).await?
        };
        let scale11 = rusttype::Scale::uniform(16f32);
        let scale12 = rusttype::Scale::uniform(17f32);
        let scale18 = rusttype::Scale::uniform(23f32);

        let (mut background, pokemon) = match self.raid.pokemon_id {
            Some(pkmn_id) if pkmn_id > 0 => {
                // $mBg = imagecreatefrompng("images/msg-bgs/msg-raid-big-t" . $v_team . ".png");
                let path = format!("{}images/msg-bgs/msg-raid-big-t{}{}.png", CONFIG.images.sender, self.raid.team_id.get_id(), if self.raid.ex_raid_eligible { "-ex" } else { "" });
                let mut background = open_image(&path).await?;

                // $mPoke = imagecreatefrompng("../../assets/img/pkmns/shuffle/" . $v_pkmnid . ".png");
                let pokemon = match self.raid.form {
                    Some(form) if form > 0 => {
                        let image = format!("{}img/pkmns/shuffle/{}-{}.png",
                            CONFIG.images.assets,
                            pkmn_id,
                            form
                        );
                        match open_image(&image).await {
                            Ok(img) => img,
                            Err(_) => {
                                let image = format!("{}img/pkmns/shuffle/{}.png",
                                    CONFIG.images.assets,
                                    pkmn_id
                                );
                                open_image(&image).await?
                            },
                        }
                    },
                    _ => {
                        let image = format!("{}img/pkmns/shuffle/{}.png",
                            CONFIG.images.assets,
                            pkmn_id
                        );
                        open_image(&image).await?
                    },
                };

                // imagettftext($mBg, 12, 0, 82, 71, 0x00000000, $f_cal2, $v_end);
                let v_end = Local.timestamp(self.raid.end, 0);
                imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 82, 59, scale12, &f_cal2, &v_end.format("%T").to_string());

                // $dm = imagettfbbox(12, 0, $f_cal2, $v_str);
                // imagettftext($mBg, 12, 0, 140 - (abs($dm[4] - $dm[6]) / 2), 100, 0x00000000, $f_cal2, $v_str);
                let text = format!("PL {}", self.raid.cp.unwrap_or_else(|| 0));
                let dm = get_text_width(&f_cal2, scale12, &text);
                imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 140 - (dm / 2) as u32, 88, scale12, &f_cal2, &text);

                // $dm = imagettfbbox(11, 0, $f_cal1, strtoupper($m_move1));
                // imagettftext($mBg, 11, 0, 80 - (abs($dm[4] - $dm[6]) / 2), 123, 0x00000000, $f_cal1, strtoupper($m_move1));
                let m_move1 = match self.raid.move_1 {
                    Some(i) => MOVES.read().await.get(&i).map(|s| s.to_uppercase()),
                    None => None,
                }.unwrap_or_else(|| String::from("-"));
                let dm = get_text_width(&f_cal1, scale11, &m_move1);
                imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 80 - (dm / 2) as u32, 111, scale11, &f_cal1, &m_move1);
                // $dm = imagettfbbox(11, 0, $f_cal1, strtoupper($m_move2));
                // imagettftext($mBg, 11, 0, 200 - (abs($dm[4] - $dm[6]) / 2), 123, 0x00000000, $f_cal1, strtoupper($m_move2));
                let m_move2 = match self.raid.move_2 {
                    Some(i) => MOVES.read().await.get(&i).map(|s| s.to_uppercase()),
                    None => None,
                }.unwrap_or_else(|| String::from("-"));
                let dm = get_text_width(&f_cal1, scale11, &m_move2);
                imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 200 - (dm / 2) as u32, 111, scale11, &f_cal1, &m_move2);

                // imagettftext($mBg, 18, 0, 63, 25, 0x00000000, $f_cal2, $p_name);
                let name = LIST.read().await.get(&pkmn_id).map(|p| p.name.to_uppercase()).unwrap_or_else(String::new);
                imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 63, 7, scale18, &f_cal2, &name);
                if let Some(id) = self.raid.form {
                    if let Some(form_name) = FORMS.read().await.get(&id) {
                        let dm = get_text_width(&f_cal2, scale18, &name);
                        imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 73 + dm as u32, 7, scale11, &f_cal2, &format!("({})", form_name));
                    }
                }

                (background, pokemon)
            },
            _ => {
                let mut background = {
                    let path = format!("{}images/msg-bgs/msg-raid-sm-t{}{}.png", CONFIG.images.sender, self.raid.team_id.get_id(), if self.raid.ex_raid_eligible { "-ex" } else { "" });
                    open_image(&path).await?
                };
                let pokemon = {
                    let path = format!("{}images/raid_{}.png", CONFIG.images.sender, self.raid.level);
                    open_image(&path).await?
                };

                // imagettftext($mBg, 12, 0, 82, 71, 0x00000000, $f_cal2, $v_battle);
                let v_battle = Local.timestamp(self.raid.start, 0);
                imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 82, 59, scale12, &f_cal2, &v_battle.format("%T").to_string());

                // imagettftext($mBg, 18, 0, 63, 25, 0x00000000, $f_cal2, $p_name);
                imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 63, 7, scale18, &f_cal2, &format!("RAID liv. {}", self.raid.level));

                (background, pokemon)
            },
        };

        image::imageops::overlay(&mut background, &pokemon, 5, 5);

        // imagettftext($mBg, 12, 0, 63, 47, 0x00000000, $f_cal2, (strlen($v_name) > 26 ? substr($v_name, 0, 25) . ".." : ($v_name == "" ? "-" : $v_name)));
        imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 63, 35, scale12, &f_cal2, &truncate_str(&self.raid.gym_name, 30, '-'));
    
        // imagecopymerge($mBg, $mMap, 0, ($v_pkmnid == 0 ? 83 : 136), 0, 0, 280, 101, 100);
        image::imageops::overlay(&mut background, &map, 0, if self.raid.pokemon_id.and_then(|i| if i > 0 { Some(i) } else { None }).is_none() { 83 } else { 136 });

        save_image(background, &img_path_str).await
    }

    async fn update_stats(&self, conn: Conn) -> Result<Conn, ()> {
        let query = format!("INSERT INTO bot_sent_raid (raid_id, sent) VALUES ('{}', 1) ON DUPLICATE KEY UPDATE sent = sent + 1", match self.raid.pokemon_id {
            Some(id) if id > 0 => { format!("p{}", id) },
            _ => format!("l{}", self.raid.level),
        });
        let res = conn.query(query).await.map_err(|e| error!("MySQL query error: {}", e))?;
        res.drop_result().await.map_err(|e| error!("MySQL drop result error: {}", e))
    }
}

#[derive(Debug)]
pub struct QuestMessage {
    pub quest: Quest,
    pub debug: Option<String>,
}

#[async_trait]
impl Message for QuestMessage {
    fn get_latitude(&self) -> f64 {
        self.quest.latitude
    }

    fn get_longitude(&self) -> f64 {
        self.quest.longitude
    }

    async fn get_caption(&self) -> Result<String, ()> {
        Err(())
    }

    async fn get_image(&self, _map: image::DynamicImage) -> Result<Vec<u8>, ()> {
        Err(())
    }

    async fn update_stats(&self, conn: Conn) -> Result<Conn, ()> {
        // TODO: impl quest stats
        Ok(conn)
    }
}

#[derive(Debug)]
pub struct InvasionMessage {
    pub invasion: Pokestop,
    pub debug: Option<String>,
}

#[async_trait]
impl Message for InvasionMessage {
    fn get_latitude(&self) -> f64 {
        self.invasion.latitude
    }

    fn get_longitude(&self) -> f64 {
        self.invasion.longitude
    }

    async fn get_caption(&self) -> Result<String, ()> {
        if let Some(timestamp) = self.invasion.incident_expire_timestamp {
            let caption = format!("{} {}\n{} {}\n{} {}",
                String::from_utf8(vec![0xC2, 0xAE]).map_err(|e| error!("error parsing R icon: {}", e))?,
                match self.invasion.grunt_type {
                    Some(id) => GRUNTS.read().await.get(&id).map(|grunt| grunt.name.clone()),
                    None => None,
                }.unwrap_or_else(String::new),
                String::from_utf8(vec![0xf0, 0x9f, 0x93, 0x8d]).map_err(|e| error!("error parsing POI icon: {}", e))?,
                self.invasion.name,
                String::from_utf8(vec![0xf0, 0x9f, 0x95, 0x92]).map_err(|e| error!("error parsing clock icon: {}", e))?,
                Local.timestamp(timestamp, 0).format("%T").to_string()
            );

            Ok(match self.debug {
                Some(ref s) => format!("{}\n\n{}", caption, s),
                None => caption,
            })
        }
        else {
            Err(())
        }
    }

    async fn get_image(&self, map: image::DynamicImage) -> Result<Vec<u8>, ()> {
        let now = Local::now();
        let img_path_str = format!("{}img_sent/invasion_{}_{}_{}.png", CONFIG.images.bot, now.format("%Y%m%d%H").to_string(), self.invasion.pokestop_id, self.invasion.grunt_type.map(|id| id.to_string()).unwrap_or_else(String::new));
        let img_path = Path::new(&img_path_str);

        if img_path.exists() {
            let mut image = File::open(&img_path).await.map_err(|e| error!("error opening invasion image {}: {}", img_path_str, e))?;
            let mut bytes = Vec::new();
            image.read_to_end(&mut bytes).await.map_err(|e| error!("error reading invasion image {}: {}", img_path_str, e))?;
            return Ok(bytes);
        }

        // let f_cal1 = {
        //     let font = format!("{}fonts/calibri.ttf", CONFIG.images.sender);
        //     open_font(&font).await?
        // };
        let f_cal2 = {
            let font = format!("{}fonts/calibrib.ttf", CONFIG.images.sender);
            open_font(&font).await?
        };
        // let scale11 = rusttype::Scale::uniform(16f32);
        let scale12 = rusttype::Scale::uniform(17f32);
        let scale13 = rusttype::Scale::uniform(18f32);
        // let scale18 = rusttype::Scale::uniform(23f32);

        let mut background = {
            let path = format!("{}images/msg-bgs/msg-invasion.png", CONFIG.images.sender);
            open_image(&path).await?
        };

        if let Some(id) = self.invasion.grunt_type {
            let lock = GRUNTS.read().await;
            if let Some(grunt) = lock.get(&id) {
                if let Some(sex) = &grunt.sex {
                    let icon = {
                        let path = format!("{}img/grunts/{}.png", CONFIG.images.assets, sex);
                        open_image(&path).await?
                    };
                    image::imageops::overlay(&mut background, &icon, 5, 5);
                }

                if let Some(element) = &grunt.element {
                    let icon = {
                        let path = format!("{}img/pkmns/types/{}{}.png", CONFIG.images.assets, &element[0..1].to_uppercase(), &element[1..]);
                        open_image(&path).await?
                    };
                    let icon = image::DynamicImage::ImageRgba8(image::imageops::resize(&icon, 24, 24, image::FilterType::Triangle));
                    image::imageops::overlay(&mut background, &icon, 32, 32);
                }
            }
        }

        imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 63, 7, scale13, &f_cal2, &truncate_str(&self.invasion.name, 25, '-'));

        if let Some(timestamp) = self.invasion.incident_expire_timestamp {
            let v_exit = Local.timestamp(timestamp, 0);
            imageproc::drawing::draw_text_mut(&mut background, image::Rgba::<u8>([0, 0, 0, 0]), 82, 34, scale12, &f_cal2, &v_exit.format("%T").to_string());
        }

        image::imageops::overlay(&mut background, &map, 0, 58);

        save_image(background, &img_path_str).await
    }

    async fn update_stats(&self, conn: Conn) -> Result<Conn, ()> {
        // TODO: impl invasions stats
        Ok(conn)
    }
}

#[derive(Debug)]
pub struct WeatherMessage {
    pub old_weather: Weather,
    pub new_weather: Weather,
    pub position: Option<(f64, f64)>,
    pub debug: Option<String>,
}

#[async_trait]
impl Message for WeatherMessage {
    fn get_latitude(&self) -> f64 {
        match self.position {
            Some((lat, _)) => lat,
            None => self.new_weather.latitude,
        }
    }

    fn get_longitude(&self) -> f64 {
        match self.position {
            Some((_, lon)) => lon,
            None => self.new_weather.longitude,
        }
    }

    async fn get_caption(&self) -> Result<String, ()> {
        Ok(format!("{} Meteo cambiato nella cella\n{}\nvecchio: {:#?}\nnuovo: {:#?}",
            String::from_utf8(vec![0xE2, 0x9B, 0x85]).map_err(|e| error!("error encoding meteo icon: {}", e))?,
            self.old_weather.diff(&self.new_weather),
            self.old_weather,
            self.new_weather))
    }

    async fn get_image(&self, map: image::DynamicImage) -> Result<Vec<u8>, ()> {
        let mut out = Vec::new();
        map.write_to(&mut out, image::ImageOutputFormat::PNG).map_err(|e| error!("error converting weather map image: {}", e))?;
        Ok(out)
    }

    async fn update_stats(&self, conn: Conn) -> Result<Conn, ()> {
        // TODO: impl weather changes stats
        Ok(conn)
    }
}
