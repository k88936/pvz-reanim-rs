pub const DEFAULT_FIELD_PLACEHOLDER: f32 = -10000.0;

#[derive(Debug, Clone)]
pub struct ReanimatorTransform {
    pub trans_x: f32,
    pub trans_y: f32,
    pub skew_x: f32,
    pub skew_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub frame: f32,
    pub alpha: f32,
    pub image: Option<String>,
    pub font: Option<String>,
    pub text: String,
}

impl Default for ReanimatorTransform {
    fn default() -> Self {
        Self {
            trans_x: DEFAULT_FIELD_PLACEHOLDER,
            trans_y: DEFAULT_FIELD_PLACEHOLDER,
            skew_x: DEFAULT_FIELD_PLACEHOLDER,
            skew_y: DEFAULT_FIELD_PLACEHOLDER,
            scale_x: DEFAULT_FIELD_PLACEHOLDER,
            scale_y: DEFAULT_FIELD_PLACEHOLDER,
            frame: DEFAULT_FIELD_PLACEHOLDER,
            alpha: DEFAULT_FIELD_PLACEHOLDER,
            image: None,
            font: None,
            text: String::new(),
        }
    }
}

impl ReanimatorTransform {
    pub fn fill_in_missing_data(&mut self, prev: &Self) {
        if self.trans_x == DEFAULT_FIELD_PLACEHOLDER {
            self.trans_x = prev.trans_x;
        }
        if self.trans_y == DEFAULT_FIELD_PLACEHOLDER {
            self.trans_y = prev.trans_y;
        }
        if self.skew_x == DEFAULT_FIELD_PLACEHOLDER {
            self.skew_x = prev.skew_x;
        }
        if self.skew_y == DEFAULT_FIELD_PLACEHOLDER {
            self.skew_y = prev.skew_y;
        }
        if self.scale_x == DEFAULT_FIELD_PLACEHOLDER {
            self.scale_x = prev.scale_x;
        }
        if self.scale_y == DEFAULT_FIELD_PLACEHOLDER {
            self.scale_y = prev.scale_y;
        }
        if self.frame == DEFAULT_FIELD_PLACEHOLDER {
            self.frame = prev.frame;
        }
        if self.alpha == DEFAULT_FIELD_PLACEHOLDER {
            self.alpha = prev.alpha;
        }
        if self.image.is_none() {
            self.image = prev.image.clone();
        }
        if self.font.is_none() {
            self.font = prev.font.clone();
        }
        if self.text.is_empty() {
            self.text.clone_from(&prev.text);
        }
    }
}
