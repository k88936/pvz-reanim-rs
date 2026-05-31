use crate::transform::ReanimatorTransform;

#[derive(Debug, Clone)]
pub struct ReanimatorTrack {
    pub name: String,
    pub transforms: Vec<ReanimatorTransform>,
}

impl ReanimatorTrack {
    pub fn fill_in_missing_data(&mut self) {
        let mut prev = ReanimatorTransform {
            trans_x: 0.0,
            trans_y: 0.0,
            skew_x: 0.0,
            skew_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            frame: 0.0,
            alpha: 1.0,
            image: None,
            font: None,
            text: String::new(),
        };
        for t in &mut self.transforms {
            t.fill_in_missing_data(&prev);
            // update prev AFTER filling
            prev = t.clone();
        }
    }
}
