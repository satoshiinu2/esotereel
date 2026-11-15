use glam::{Mat4, Vec3, camera::lh::proj::directx};

/// カメラ情報です。
///
/// 視点・回転・投影種別（正射影/透視投影）を持ち、ビュー行列と射影行列を
/// 生成します。レンダリング時にカメラ変換を適用するために使用されます。
#[repr(C)]
#[derive(Debug)]
pub struct CameraInfo {
    /// カメラ位置
    position: Vec3,
    /// 回転（度）
    rotation: Vec3,
    /// 正射影モードかどうか
    is_orthographic: bool,
    /// 正射影方向
    orthographic_direction: Direction,
    /// スケール係数
    scale_factor: f32,
    /// 視野角（度）
    fov: f32,
}

impl CameraInfo {
    /// ビュー行列を返します。
    ///
    /// 正射影モードの場合は方向に合わせた固定回転を適用し、
    /// 3Dモードの場合はオイラー角から回転を生成します。
    /// 左手系への変換と平行移動を適用した行列を返します。
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

    /// 射影行列を返します。
    ///
    /// 正射影モードの場合は 2D 用の平行投影行列（ズームレベルを考慮）を、
    /// 3Dモードの場合は透視投影行列を返します。
    pub fn get_proj_mat(&self, screen_size: [f32; 2]) -> Mat4 {
        let half_w = screen_size[0] / 2.0;
        let half_h = screen_size[1] / 2.0;
        let aspect = screen_size[0] / screen_size[1];
        let fov = self.fov.to_radians();

        if self.is_orthographic {
            // 2D用の平行投影行列（ズームレベルを考慮）
            let ortho_mat =
                directx::orthographic(-half_w, half_w, half_h, -half_h, -1000.0, 1000.0);
            let scale_mat = Mat4::from_scale(Vec3::new(self.scale_factor, self.scale_factor, 1.0));

            scale_mat * ortho_mat
        } else {
            // 3D用の透視投影行列
            directx::perspective(fov, aspect, 0.1, 1000.0)
        }
    }
}

/// カメラの向きです。
///
/// 正射影モードで使用される固定方向を表現します。
#[derive(Clone, Copy, Debug)]
#[repr(C)]

pub enum Direction {
    /// 前方
    Front,
    /// 後方
    Back,
    /// 上方
    Top,
    /// 下方
    Bottom,
    /// 左方
    Left,
    /// 右方
    Right,
}

impl Direction {
    /// 回転行列を返します。
    ///
    /// 各方向に対応する固定回転行列を返します。
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

#[cfg(test)]
#[path = "../../tests/unit/project/camera_preview_tests.rs"]
mod preview_tests;
