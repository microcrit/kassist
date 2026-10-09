use components::Panel;
use stonecutter::{Plugin, PluginError, PluginId, PluginManifest, Version};

pub const PYTHON_BRIDGE: stonecutter::PythonBridge = stonecutter::python_bridge!(
    "shadeproject",
    stonecutter::requirements_txt![
        "--extra-index-url https://download.pytorch.org/whl/cu128",
        "torch==2.11.0+cu128",
        "torchvision==0.26.0+cu128",
        "timm>=1.0",
        "opencv-python-headless>=4.10",
        "numpy>=2",
    ],
);

const MAX_LIGHTS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitVector {
    x: f32,
    y: f32,
    z: f32,
}

impl UnitVector {
    pub fn new(x: f32, y: f32, z: f32) -> Result<Self, VectorError> {
        let magnitude = (x * x + y * y + z * z).sqrt();
        if !magnitude.is_finite() || magnitude <= f32::EPSILON {
            return Err(VectorError::ZeroOrNonFinite);
        }
        Ok(Self {
            x: x / magnitude,
            y: y / magnitude,
            z: z / magnitude,
        })
    }
    pub const fn components(self) -> (f32, f32, f32) {
        (self.x, self.y, self.z)
    }
    fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VectorError {
    ZeroOrNonFinite,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DepthMap {
    width: usize,
    height: usize,
    values: Vec<f32>,
}

impl DepthMap {
    pub fn new(width: usize, height: usize, values: Vec<f32>) -> Result<Self, MapError> {
        if width == 0
            || height == 0
            || values.len() != width * height
            || values.iter().any(|value| !value.is_finite())
        {
            return Err(MapError::InvalidDimensions);
        }
        Ok(Self {
            width,
            height,
            values,
        })
    }
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    pub fn normals(&self, depth_scale: f32) -> Result<NormalMap, MapError> {
        if !depth_scale.is_finite() || depth_scale <= 0.0 {
            return Err(MapError::InvalidScale);
        }
        let mut normals = Vec::with_capacity(self.values.len());
        for y in 0..self.height {
            for x in 0..self.width {
                let left = self.value(x.saturating_sub(1), y);
                let right = self.value((x + 1).min(self.width - 1), y);
                let up = self.value(x, y.saturating_sub(1));
                let down = self.value(x, (y + 1).min(self.height - 1));
                normals.push(
                    UnitVector::new(
                        -(right - left) * depth_scale,
                        -(down - up) * depth_scale,
                        2.0,
                    )
                    .map_err(|_| MapError::InvalidScale)?,
                );
            }
        }
        Ok(NormalMap {
            width: self.width,
            height: self.height,
            normals,
        })
    }
    fn value(&self, x: usize, y: usize) -> f32 {
        self.values[y * self.width + x]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalMap {
    width: usize,
    height: usize,
    normals: Vec<UnitVector>,
}

impl NormalMap {
    pub fn new(width: usize, height: usize, normals: Vec<UnitVector>) -> Result<Self, MapError> {
        if width == 0 || height == 0 || normals.len() != width * height {
            return Err(MapError::InvalidDimensions);
        }
        Ok(Self {
            width,
            height,
            normals,
        })
    }
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
    pub fn normals(&self) -> &[UnitVector] {
        &self.normals
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderMap {
    width: usize,
    height: usize,
    values: Vec<f32>,
}

impl RenderMap {
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapError {
    InvalidDimensions,
    InvalidScale,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectionalLight {
    pub direction: UnitVector,
    pub intensity: f32,
}

impl DirectionalLight {
    pub fn new(direction: UnitVector, intensity: f32) -> Result<Self, LightingError> {
        if !intensity.is_finite() || intensity < 0.0 {
            return Err(LightingError::InvalidIntensity);
        }
        Ok(Self {
            direction,
            intensity,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lighting {
    lights: Vec<DirectionalLight>,
    ambient: f32,
    exposure: f32,
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            lights: vec![DirectionalLight::new(
                UnitVector::new(-1.0, -1.0, 1.0).expect("non-zero constant"),
                1.0,
            )
            .expect("valid constant")],
            ambient: 0.15,
            exposure: 1.0,
        }
    }
}

impl Lighting {
    pub fn lights(&self) -> &[DirectionalLight] {
        &self.lights
    }
    pub fn ambient(&self) -> f32 {
        self.ambient
    }
    pub fn set_ambient(&mut self, ambient: f32) -> Result<(), LightingError> {
        if !ambient.is_finite() || !(0.0..=1.0).contains(&ambient) {
            return Err(LightingError::InvalidAmbient);
        }
        self.ambient = ambient;
        Ok(())
    }
    pub fn set_exposure(&mut self, exposure: f32) -> Result<(), LightingError> {
        if !exposure.is_finite() || exposure <= 0.0 {
            return Err(LightingError::InvalidExposure);
        }
        self.exposure = exposure;
        Ok(())
    }
    pub fn add_light(&mut self, light: DirectionalLight) -> Result<(), LightingError> {
        if self.lights.len() == MAX_LIGHTS {
            return Err(LightingError::TooManyLights);
        }
        self.lights.push(light);
        Ok(())
    }

    pub fn intensity(&self, normal: UnitVector) -> f32 {
        let direct: f32 = self
            .lights
            .iter()
            .map(|light| normal.dot(light.direction).max(0.0) * light.intensity)
            .sum();
        (self.ambient + direct * self.exposure).clamp(0.0, 1.0)
    }
    pub fn render_map(&self, normals: &NormalMap) -> RenderMap {
        RenderMap {
            width: normals.width,
            height: normals.height,
            values: normals
                .normals
                .iter()
                .map(|normal| self.intensity(*normal))
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightingError {
    InvalidIntensity,
    InvalidAmbient,
    InvalidExposure,
    TooManyLights,
}

pub struct ShadeProjectPlugin {
    manifest: PluginManifest,
    panel: Panel,
    lighting: Lighting,
}

impl Default for ShadeProjectPlugin {
    fn default() -> Self {
        let panel = Panel::new("Shade Project", |ui| {
            ui.row(|ui| {
                ui.button("Reset light", "reset_light");
                ui.button("Increase exposure", "increase_exposure");
            });
        })
        .expect("static panel is valid");
        Self {
            manifest: PluginManifest::new(
                PluginId::new("cc.hellings.shadeproject").expect("static ID is valid"),
                "Shade Project",
                Version::new(0, 2, 0),
                Version::new(5, 2, 0),
            )
            .expect("static manifest is valid"),
            panel,
            lighting: Lighting::default(),
        }
    }
}

impl ShadeProjectPlugin {
    pub fn lighting(&self) -> &Lighting {
        &self.lighting
    }
}

impl Plugin for ShadeProjectPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
    fn panel(&self) -> &Panel {
        &self.panel
    }
    fn invoke(&mut self, action: &str) -> Result<(), PluginError> {
        match action {
            "reset_light" => {
                self.lighting = Lighting::default();
                Ok(())
            }
            "increase_exposure" => self
                .lighting
                .set_exposure(self.lighting.exposure * 1.1)
                .map_err(|error| PluginError::Failed(format!("{error:?}"))),
            _ => Err(PluginError::UnknownAction(action.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DepthMap, DirectionalLight, Lighting, NormalMap, UnitVector};
    #[test]
    fn depth_map_produces_canvas_sized_normals() {
        let map = DepthMap::new(2, 2, vec![0.0, 1.0, 0.0, 1.0]).unwrap();
        assert_eq!(map.normals(1.0).unwrap().dimensions(), (2, 2));
    }
    #[test]
    fn facing_surface_is_lighter_than_away_surface() {
        let light = Lighting::default();
        let facing = UnitVector::new(-1.0, -1.0, 1.0).unwrap();
        let away = UnitVector::new(1.0, 1.0, -1.0).unwrap();
        assert!(light.intensity(facing) > light.intensity(away));
    }
    #[test]
    fn render_map_preserves_dimensions() {
        let normals =
            NormalMap::new(2, 1, vec![UnitVector::new(0.0, 0.0, 1.0).unwrap(); 2]).unwrap();
        assert_eq!(
            Lighting::default().render_map(&normals).dimensions(),
            (2, 1)
        );
    }
    #[test]
    fn lighting_rig_accepts_multiple_lights() {
        let mut lighting = Lighting::default();
        lighting
            .add_light(DirectionalLight::new(UnitVector::new(1.0, 0.0, 1.0).unwrap(), 0.5).unwrap())
            .unwrap();
        assert_eq!(lighting.lights().len(), 2);
    }
}
