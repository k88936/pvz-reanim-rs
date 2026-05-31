use crate::transform::ReanimatorTransform;

#[derive(Debug, Clone, Copy)]
pub struct ReanimatorFrameTime {
    pub fraction: f32,
    pub frame_before: i32,
    pub frame_after: i32,
}

impl ReanimatorFrameTime {
    pub fn from_anim_time(
        anim_time: f32,
        frame_start: i32,
        frame_count: i32,
        use_full_last_frame: bool,
    ) -> Self {
        let frame_count_for_interp = if use_full_last_frame {
            frame_count
        } else {
            frame_count - 1
        };
        let anim_position = frame_start as f32 + anim_time * frame_count_for_interp as f32;
        let frame_before_float = anim_position.floor();
        let fraction = anim_position - frame_before_float;
        let mut frame_before = frame_before_float as i32;
        let mut frame_after = frame_before + 1;

        let last_frame = frame_start + frame_count - 1;
        if frame_before >= last_frame {
            frame_before = last_frame;
            frame_after = last_frame;
        }

        Self {
            fraction,
            frame_before,
            frame_after,
        }
    }
}

pub fn lerp_transform(
    before: &ReanimatorTransform,
    after: &ReanimatorTransform,
    fraction: f32,
) -> ReanimatorTransform {
    ReanimatorTransform {
        trans_x: before.trans_x + fraction * (after.trans_x - before.trans_x),
        trans_y: before.trans_y + fraction * (after.trans_y - before.trans_y),
        skew_x: before.skew_x + fraction * (after.skew_x - before.skew_x),
        skew_y: before.skew_y + fraction * (after.skew_y - before.skew_y),
        scale_x: before.scale_x + fraction * (after.scale_x - before.scale_x),
        scale_y: before.scale_y + fraction * (after.scale_y - before.scale_y),
        alpha: before.alpha + fraction * (after.alpha - before.alpha),
        frame: before.frame,
        image: before.image.clone(),
        font: before.font.clone(),
        text: before.text.clone(),
    }
}
