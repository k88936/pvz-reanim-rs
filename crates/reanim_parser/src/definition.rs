use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{ReanimError, Result};
use crate::track::ReanimatorTrack;
use crate::transform::ReanimatorTransform;

#[derive(Debug, Clone)]
pub struct ReanimatorDefinition {
    pub tracks: Vec<ReanimatorTrack>,
    pub fps: f32,
}

fn parse_transform_field(tag: &str, value: &str, transform: &mut ReanimatorTransform) {
    let trimmed = value.trim();
    match tag {
        "x" => {
            if let Ok(v) = trimmed.parse() {
                transform.trans_x = v;
            }
        }
        "y" => {
            if let Ok(v) = trimmed.parse() {
                transform.trans_y = v;
            }
        }
        "kx" => {
            if let Ok(v) = trimmed.parse() {
                transform.skew_x = v;
            }
        }
        "ky" => {
            if let Ok(v) = trimmed.parse() {
                transform.skew_y = v;
            }
        }
        "sx" => {
            if let Ok(v) = trimmed.parse() {
                transform.scale_x = v;
            }
        }
        "sy" => {
            if let Ok(v) = trimmed.parse() {
                transform.scale_y = v;
            }
        }
        "f" => {
            if let Ok(v) = trimmed.parse() {
                transform.frame = v;
            }
        }
        "a" => {
            if let Ok(v) = trimmed.parse() {
                transform.alpha = v;
            }
        }
        "i" => {
            if !trimmed.is_empty() {
                transform.image = Some(trimmed.to_string());
            }
        }
        "font" => {
            if !trimmed.is_empty() {
                transform.font = Some(trimmed.to_string());
            }
        }
        "text" => {
            transform.text = trimmed.to_string();
        }
        _ => {}
    }
}

impl ReanimatorDefinition {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut reader = Reader::from_file(path.as_ref())?;
        let mut buf = Vec::new();
        let mut current_track: Option<ReanimatorTrack> = None;
        let mut current_transform: Option<ReanimatorTransform> = None;
        let mut text_content = String::new();
        let mut def = Self {
            tracks: Vec::new(),
            fps: 12.0,
        };

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let tag =
                        std::str::from_utf8(e.name().as_ref()).unwrap_or_default().to_string();

                    match tag.as_str() {
                        "track" => {
                            current_track = Some(ReanimatorTrack {
                                name: String::new(),
                                transforms: Vec::new(),
                            });
                        }
                        "t" => {
                            if current_track.is_some() {
                                current_transform = Some(ReanimatorTransform::default());
                            }
                        }
                        "x" | "y" | "kx" | "ky" | "sx" | "sy" | "f" | "a" | "i" | "font"
                        | "text" | "name" => {
                            text_content.clear();
                        }
                        _ => {}
                    }
                }

                Ok(Event::Empty(ref e)) => {
                    let tag =
                        std::str::from_utf8(e.name().as_ref()).unwrap_or_default().to_string();

                    if tag == "t" && current_track.is_some() {
                        current_transform = Some(ReanimatorTransform::default());
                        if let Some(transform) = current_transform.take() {
                            if let Some(ref mut track) = current_track {
                                track.transforms.push(transform);
                            }
                        }
                    }
                }

                Ok(Event::Text(ref e)) => {
                    text_content = e.unescape()?.to_string();
                }

                Ok(Event::End(ref e)) => {
                    let tag =
                        std::str::from_utf8(e.name().as_ref()).unwrap_or_default().to_string();

                    match tag.as_str() {
                        "track" => {
                            if let Some(track) = current_track.take() {
                                def.tracks.push(track);
                            }
                        }
                        "t" => {
                            if let Some(transform) = current_transform.take() {
                                if let Some(ref mut track) = current_track {
                                    track.transforms.push(transform);
                                }
                            }
                        }
                        "name" => {
                            if let Some(ref mut track) = current_track {
                                track.name = text_content.trim().to_string();
                            }
                        }
                        "fps" => {
                            def.fps = text_content.trim().parse().unwrap_or(12.0);
                        }
                        "x" | "y" | "kx" | "ky" | "sx" | "sy" | "f" | "a" | "i" | "font"
                        | "text" => {
                            if let Some(ref mut t) = current_transform {
                                parse_transform_field(&tag, &text_content, t);
                            }
                        }
                        _ => {}
                    }
                }

                Ok(Event::Eof) => break,

                Err(e) => return Err(ReanimError::Xml(e)),

                _ => {}
            }
            buf.clear();
        }

        for track in &mut def.tracks {
            track.fill_in_missing_data();
        }

        Ok(def)
    }

    pub fn frame_count(&self) -> i32 {
        self.tracks.first().map_or(0, |t| t.transforms.len() as i32)
    }

    /// Returns the set of unique image names referenced by any track in this definition.
    pub fn referenced_images(&self) -> Vec<String> {
        let mut images: Vec<String> = Vec::new();
        for track in &self.tracks {
            for tx in &track.transforms {
                if let Some(ref img) = tx.image {
                    if !images.contains(img) {
                        images.push(img.clone());
                    }
                }
            }
        }
        images.sort();
        images
    }
}
