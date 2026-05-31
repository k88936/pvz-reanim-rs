use crate::transform::ReanimatorTransform;

const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;

pub fn from_transform(transform: &ReanimatorTransform) -> glam::Mat3 {
    let a_skew_x = -DEG_TO_RAD * transform.skew_x;
    let a_skew_y = -DEG_TO_RAD * transform.skew_y;

    let m = glam::Mat3::from_cols(
        glam::vec3(a_skew_x.cos() * transform.scale_x, -a_skew_x.sin() * transform.scale_x, 0.0),
        glam::vec3(a_skew_y.sin() * transform.scale_y, a_skew_y.cos() * transform.scale_y, 0.0),
        glam::vec3(transform.trans_x, transform.trans_y, 1.0),
    );
    log::debug!(
        "[from_transform] skew=({}, {}) scale=({}, {}) trans=({}, {})",
        transform.skew_x, transform.skew_y,
        transform.scale_x, transform.scale_y,
        transform.trans_x, transform.trans_y,
    );
    log::debug!(
        "[from_transform] m00={} m01={} m02={}", m.x_axis.x, m.y_axis.x, m.z_axis.x
    );
    log::debug!(
        "[from_transform] m10={} m11={} m12={}", m.x_axis.y, m.y_axis.y, m.z_axis.y
    );
    m
}
