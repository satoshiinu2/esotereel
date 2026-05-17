use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Debug)]
pub struct CameraInfo {
    position: Vec3,
    rotation: Vec3, // degree
    is_orthographic: bool,
    orthographic_direction: Direction,
    scale_factor: f32,
    fov: f32, // degree
}

impl CameraInfo {
    pub fn get_view_mat(&self) -> Mat4 {
        let rot_mat = if self.is_orthographic {
            // 2Dモード: 方向に合わせた固定回転を適用
            self.orthographic_direction.get_rotation_mat()
        } else {
            // 3Dモード: オイラー角から回転を生成
            Mat4::from_euler(
                glam::EulerRot::XYZ,
                self.rotation.x.to_radians(),
                self.rotation.y.to_radians(),
                self.rotation.z.to_radians(),
            )
        };

        // 左手系への変換と平行移動を適用
        let trans_mat =
            Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0)) * Mat4::from_translation(-self.position);

        rot_mat * trans_mat
    }

    pub fn get_proj_mat(&self, screen_size: [f32; 2]) -> Mat4 {
        let half_w = screen_size[0] / 2.0;
        let half_h = screen_size[1] / 2.0;
        let aspect = screen_size[0] / screen_size[1];
        let fov = self.fov.to_radians();

        if self.is_orthographic {
            // 2D用の平行投影行列（ズームレベルを考慮）
            let ortho_mat = Mat4::orthographic_lh(-half_w, half_w, half_h, -half_h, -1000.0, 1000.0);
            let scale_mat = Mat4::from_scale(Vec3::new(self.scale_factor, self.scale_factor, 1.0));

            scale_mat * ortho_mat
        } else {
            // 3D用の透視投影行列
            Mat4::perspective_lh(fov, aspect, 0.1, 1000.0)
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]

pub enum Direction {
    Front,
    Back,
    Top,
    Bottom,
    Left,
    Right,
}

impl Direction {
    pub fn get_rotation_mat(&self) -> Mat4 {
        match self {
            Direction::Front => Mat4::IDENTITY,
            Direction::Back => Mat4::from_rotation_y(180.0_f32.to_radians()),
            Direction::Top => Mat4::from_rotation_x(90.0_f32.to_radians()),
            Direction::Bottom => Mat4::from_rotation_x(-90.0_f32.to_radians()),
            Direction::Left => Mat4::from_rotation_y(90.0_f32.to_radians()),
            Direction::Right => Mat4::from_rotation_y(-90.0_f32.to_radians()),
        }
    }
}
