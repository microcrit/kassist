use commonext::{ValidationError, ensure};
use components::Panel;

#[cfg(feature = "gpu")]
pub mod gpu;

#[derive(Clone, Debug, PartialEq)]
pub struct BrushRecipe {
    color: String,
    size: f64,
    opacity: f64,
}

impl BrushRecipe {
    pub fn new(color: impl Into<String>, size: f64, opacity: f64) -> Result<Self, ValidationError> {
        let color = color.into();
        ensure!(!color.trim().is_empty(), "brush.color", "must not be empty");
        ensure!(
            size.is_finite() && size > 0.0,
            "brush.size",
            "must be a positive finite number"
        );
        ensure!(
            opacity.is_finite() && (0.0..=1.0).contains(&opacity),
            "brush.opacity",
            "must be between zero and one"
        );
        Ok(Self {
            color,
            size,
            opacity,
        })
    }

    pub fn color(&self) -> &str {
        &self.color
    }
    pub fn size(&self) -> f64 {
        self.size
    }
    pub fn opacity(&self) -> f64 {
        self.opacity
    }
}

pub struct ToolDefinition {
    id: String,
    name: String,
    panel: Panel,
    brush: BrushRecipe,
}

/// A portable, raster-friendly shape description for Krita-facing tools.
/// Coordinates are expressed in document pixels and rendering is delegated to
/// the host-side Qt bridge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
    Line,
    Polygon,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapeRecipe {
    kind: ShapeKind,
    fill: Option<String>,
    stroke: Option<String>,
    stroke_width: f64,
}

impl ShapeRecipe {
    pub fn new(
        kind: ShapeKind,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: f64,
    ) -> Result<Self, ValidationError> {
        ensure!(
            fill.as_ref().is_some_and(|value| !value.trim().is_empty())
                || stroke
                    .as_ref()
                    .is_some_and(|value| !value.trim().is_empty()),
            "shape.paint",
            "needs a fill or stroke"
        );
        ensure!(
            stroke_width.is_finite() && stroke_width >= 0.0,
            "shape.stroke_width",
            "must be a non-negative finite number"
        );
        Ok(Self {
            kind,
            fill,
            stroke,
            stroke_width,
        })
    }
    pub fn kind(&self) -> ShapeKind {
        self.kind
    }
    pub fn fill(&self) -> Option<&str> {
        self.fill.as_deref()
    }
    pub fn stroke(&self) -> Option<&str> {
        self.stroke.as_deref()
    }
    pub fn stroke_width(&self) -> f64 {
        self.stroke_width
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextRecipe {
    text: String,
    family: String,
    point_size: f64,
    color: String,
}

impl TextRecipe {
    pub fn new(
        text: impl Into<String>,
        family: impl Into<String>,
        point_size: f64,
        color: impl Into<String>,
    ) -> Result<Self, ValidationError> {
        let text = text.into();
        let family = family.into();
        let color = color.into();
        ensure!(!text.is_empty(), "text.content", "must not be empty");
        ensure!(
            !family.trim().is_empty(),
            "text.family",
            "must not be empty"
        );
        ensure!(
            point_size.is_finite() && point_size > 0.0,
            "text.point_size",
            "must be a positive finite number"
        );
        ensure!(!color.trim().is_empty(), "text.color", "must not be empty");
        Ok(Self {
            text,
            family,
            point_size,
            color,
        })
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn family(&self) -> &str {
        &self.family
    }
    pub fn point_size(&self) -> f64 {
        self.point_size
    }
    pub fn color(&self) -> &str {
        &self.color
    }
}

impl ToolDefinition {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        brush: BrushRecipe,
        build: impl FnOnce(&mut components::Ui),
    ) -> Result<Self, ValidationError> {
        let id = id.into();
        ensure!(
            !id.is_empty()
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'),
            "tool.id",
            "must use lowercase ASCII letters, digits, or '-'"
        );
        let name = name.into();
        ensure!(!name.trim().is_empty(), "tool.name", "must not be empty");
        let panel = Panel::new(&name, build)?;
        Ok(Self {
            id,
            name,
            panel,
            brush,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn panel(&self) -> &Panel {
        &self.panel
    }
    pub fn brush(&self) -> &BrushRecipe {
        &self.brush
    }

    pub fn handles_action(&self, action: &str) -> bool {
        self.panel.actions().iter().any(|known| known == action)
    }
}

pub fn python_tool_api() -> &'static str {
    include_str!("../python/kassist_cakewalk.py")
}

/// Extra modules emitted beside the stable Cakewalk facade.
pub fn python_tool_extensions() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "kassist_cakewalk_gpu.py",
            include_str!("../python/kassist_cakewalk_gpu.py"),
        ),
        (
            "kassist_cakewalk_design.py",
            include_str!("../python/kassist_cakewalk_design.py"),
        ),
        (
            "kassist_cakewalk_plugs/__init__.py",
            include_str!("../python/kassist_cakewalk_plugs/__init__.py"),
        ),
        (
            "kassist_cakewalk_plugs/blur.py",
            include_str!("../python/kassist_cakewalk_plugs/blur.py"),
        ),
        (
            "kassist_cakewalk_plugs/eraser.py",
            include_str!("../python/kassist_cakewalk_plugs/eraser.py"),
        ),
        (
            "kassist_cakewalk_plugs/transform.py",
            include_str!("../python/kassist_cakewalk_plugs/transform.py"),
        ),
        (
            "kassist_cakewalk_plugs/warp.py",
            include_str!("../python/kassist_cakewalk_plugs/warp.py"),
        ),
        (
            "kassist_cakewalk_plugs/distort.py",
            include_str!("../python/kassist_cakewalk_plugs/distort.py"),
        ),
        (
            "kassist_cakewalk_plugs/smudge.py",
            include_str!("../python/kassist_cakewalk_plugs/smudge.py"),
        ),
        (
            "kassist_cakewalk_plugs/clone.py",
            include_str!("../python/kassist_cakewalk_plugs/clone.py"),
        ),
        (
            "kassist_cakewalk_plugs/fill.py",
            include_str!("../python/kassist_cakewalk_plugs/fill.py"),
        ),
        (
            "kassist_cakewalk_plugs/bounded_fill.py",
            include_str!("../python/kassist_cakewalk_plugs/bounded_fill.py"),
        ),
        (
            "kassist_cakewalk_plugs/color_picker.py",
            include_str!("../python/kassist_cakewalk_plugs/color_picker.py"),
        ),
        (
            "kassist_cakewalk_plugs/stabilizer.py",
            include_str!("../python/kassist_cakewalk_plugs/stabilizer.py"),
        ),
        (
            "kassist_cakewalk_plugs/lazy_stabilizer.py",
            include_str!("../python/kassist_cakewalk_plugs/lazy_stabilizer.py"),
        ),
        (
            "kassist_cakewalk_plugs/bounded_shading.py",
            include_str!("../python/kassist_cakewalk_plugs/bounded_shading.py"),
        ),
        (
            "kassist_cakewalk_plugs/edge_detect.py",
            include_str!("../python/kassist_cakewalk_plugs/edge_detect.py"),
        ),
        (
            "kassist_cakewalk_plugs/hair_guides.py",
            include_str!("../python/kassist_cakewalk_plugs/hair_guides.py"),
        ),
        (
            "kassist_cakewalk_plugs/shader.py",
            include_str!("../python/kassist_cakewalk_plugs/shader.py"),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::{BrushRecipe, ShapeKind, ShapeRecipe, TextRecipe, ToolDefinition};

    #[test]
    fn tool_preserves_panel_and_brush_contract() {
        let tool = ToolDefinition::new(
            "sparkle",
            "Sparkle",
            BrushRecipe::new("#ffffff", 12.0, 0.5).unwrap(),
            |ui| {
                ui.slider("Density", 0.5, 0.0, 1.0, 2, "density");
            },
        )
        .unwrap();
        assert_eq!(tool.id(), "sparkle");
        assert_eq!(tool.brush().size(), 12.0);
        assert!(tool.handles_action("density"));
    }

    #[test]
    fn tool_rejects_unstable_ids() {
        let result = ToolDefinition::new(
            "Sparkle Tool",
            "Sparkle",
            BrushRecipe::new("#fff", 1.0, 1.0).unwrap(),
            |_ui| {},
        );
        assert!(result.is_err());
    }

    #[test]
    fn design_recipes_validate_renderable_content() {
        let shape = ShapeRecipe::new(ShapeKind::Rectangle, Some("#fff".into()), None, 0.0).unwrap();
        assert_eq!(shape.kind(), ShapeKind::Rectangle);
        assert!(TextRecipe::new("Title", "Inter", 24.0, "#111").is_ok());
        assert!(ShapeRecipe::new(ShapeKind::Line, None, None, 1.0).is_err());
    }

    #[test]
    fn generated_python_tool_runtime_is_syntactically_valid() {
        let source = crate::python_tool_api();
        for fragment in [
            "class CanvasInputInterceptor",
            "class CanvasTransform",
            "def install_canvas_hook",
            "class BrushProgram",
            "def preview",
            "class PixelSurface",
            "class PixelContext",
            "class CanvasTool",
            "class ToolPanel",
        ] {
            assert!(
                source.contains(fragment),
                "tool runtime is missing {fragment}"
            );
        }
        let path = std::env::temp_dir().join(format!("kassist-cakewalk-{}.py", std::process::id()));
        std::fs::write(&path, source).unwrap();
        let output = std::process::Command::new("python3")
            .args(["-m", "py_compile", path.to_str().unwrap()])
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(
            output.status.success(),
            "tool runtime is not valid Python: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn design_runtime_is_emitted() {
        let files = crate::python_tool_extensions();
        let source = files
            .iter()
            .find(|(path, _)| *path == "kassist_cakewalk_design.py")
            .unwrap()
            .1;
        assert!(source.contains("def paint_shape"));
        assert!(source.contains("def rich_text_image"));
        assert!(source.contains("def register_font_file"));
    }

    #[test]
    fn every_builtin_engine_is_emitted_as_an_individual_plug() {
        let files = crate::python_tool_extensions();
        for name in [
            "kassist_cakewalk_plugs/blur.py",
            "kassist_cakewalk_plugs/eraser.py",
            "kassist_cakewalk_plugs/transform.py",
            "kassist_cakewalk_plugs/warp.py",
            "kassist_cakewalk_plugs/distort.py",
            "kassist_cakewalk_plugs/smudge.py",
            "kassist_cakewalk_plugs/clone.py",
            "kassist_cakewalk_plugs/fill.py",
            "kassist_cakewalk_plugs/bounded_fill.py",
            "kassist_cakewalk_plugs/color_picker.py",
            "kassist_cakewalk_plugs/stabilizer.py",
            "kassist_cakewalk_plugs/lazy_stabilizer.py",
            "kassist_cakewalk_plugs/bounded_shading.py",
            "kassist_cakewalk_plugs/edge_detect.py",
            "kassist_cakewalk_plugs/hair_guides.py",
            "kassist_cakewalk_plugs/shader.py",
        ] {
            assert!(
                files.iter().any(|(path, _)| *path == name),
                "missing {name}"
            );
        }
        for (path, source) in files {
            if path.ends_with("bounded_shading.py") {
                assert!(source.contains("class BoundedShadingEngine"));
            }
            if path.ends_with("hair_guides.py") {
                assert!(source.contains("class HairGuideEngine"));
            }
        }
    }

    #[test]
    fn user_plug_registry_is_emitted_with_the_builtin_plugs() {
        let files = crate::python_tool_extensions();
        let registry = files
            .iter()
            .find(|(path, _)| *path == "kassist_cakewalk_plugs/__init__.py")
            .expect("plug registry must be emitted")
            .1;
        for fragment in [
            "def builtin_tools",
            "def listed_tools",
            "def discover_user",
            "def load_user",
            "def user_tools",
            "def write_template",
        ] {
            assert!(
                registry.contains(fragment),
                "user plug registry is missing {fragment}"
            );
        }
    }
}
