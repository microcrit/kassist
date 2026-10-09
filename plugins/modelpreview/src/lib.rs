use components::Panel;
use stonecutter::{Plugin, PluginError, PluginId, PluginManifest, Version};

pub const PYTHON_BRIDGE: stonecutter::PythonBridge = stonecutter::python_bridge!(
    "modelpreview",
    stonecutter::requirements_txt!["numpy>=2", "trimesh>=4.6", "pygltflib>=1.16"],
);

stonecutter::export_python_bridge_version!(kassist_modelpreview_bridge_version);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
    pub roll_degrees: f32,
    pub scale: f32,
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            yaw_degrees: 0.0,
            pitch_degrees: 0.0,
            roll_degrees: 0.0,
            scale: 1.0,
        }
    }
}

impl Pose {
    pub fn matrix(self) -> [[f32; 3]; 3] {
        let (yaw_sin, yaw_cos) = self.yaw_degrees.to_radians().sin_cos();
        let (pitch_sin, pitch_cos) = self.pitch_degrees.to_radians().sin_cos();
        let (roll_sin, roll_cos) = self.roll_degrees.to_radians().sin_cos();
        let yaw = [[yaw_cos, 0.0, yaw_sin], [0.0, 1.0, 0.0], [-yaw_sin, 0.0, yaw_cos]];
        let pitch = [[1.0, 0.0, 0.0], [0.0, pitch_cos, -pitch_sin], [0.0, pitch_sin, pitch_cos]];
        let roll = [[roll_cos, -roll_sin, 0.0], [roll_sin, roll_cos, 0.0], [0.0, 0.0, 1.0]];
        scale_matrix(multiply(multiply(roll, pitch), yaw), self.scale.max(0.001))
    }
}

pub struct ModelPreviewPlugin {
    manifest: PluginManifest,
    panel: Panel,
}

impl Default for ModelPreviewPlugin {
    fn default() -> Self {
        let panel = Panel::new("Model preview", |ui| {
            ui.row(|ui| {
                ui.button("Open preview", "preview");
                ui.button("Write MakeHuman profile", "makehuman");
            });
        })
        .expect("static panel is valid");
        Self {
            manifest: PluginManifest::new(
                PluginId::new("cc.hellings.modelpreview").expect("static ID is valid"),
                "Model Preview",
                Version::new(0, 1, 0),
                Version::new(5, 2, 0),
            )
            .expect("static manifest is valid"),
            panel,
        }
    }
}

impl Plugin for ModelPreviewPlugin {
    fn manifest(&self) -> &PluginManifest { &self.manifest }
    fn panel(&self) -> &Panel { &self.panel }
    fn invoke(&mut self, action: &str) -> Result<(), PluginError> {
        match action {
            "preview" | "makehuman" => Ok(()),
            _ => Err(PluginError::UnknownAction(action.into())),
        }
    }
}

fn multiply(left: [[f32; 3]; 3], right: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut result = [[0.0; 3]; 3];
    for row in 0..3 {
        for column in 0..3 {
            result[row][column] = (0..3).map(|index| left[row][index] * right[index][column]).sum();
        }
    }
    result
}

fn scale_matrix(mut matrix: [[f32; 3]; 3], scale: f32) -> [[f32; 3]; 3] {
    for row in &mut matrix { for value in row { *value *= scale; } }
    matrix
}

#[cfg(test)]
mod tests {
    use super::Pose;
    #[test]
    fn pose_matrix_scales_identity() {
        let matrix = Pose { scale: 2.0, ..Pose::default() }.matrix();
        assert_eq!(matrix[0][0], 2.0);
        assert_eq!(matrix[1][1], 2.0);
        assert_eq!(matrix[2][2], 2.0);
    }
}
