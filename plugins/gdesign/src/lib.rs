use components::Panel;
use stonecutter::{Plugin, PluginError, PluginId, PluginManifest, Version};

pub const PYTHON_BRIDGE: stonecutter::PythonBridge =
    stonecutter::python_bridge!("gdesign", stonecutter::requirements_txt![],);

pub struct GDesignPlugin {
    manifest: PluginManifest,
    panel: Panel,
}

impl Default for GDesignPlugin {
    fn default() -> Self {
        let panel = Panel::new("Graphic design", |ui| {
            ui.button("Update rich text", "text");
        })
        .expect("static panel is valid");
        Self {
            manifest: PluginManifest::new(
                PluginId::new("cc.hellings.gdesign").expect("static ID"),
                "GDesign",
                Version::new(0, 1, 0),
                Version::new(5, 2, 0),
            )
            .expect("static manifest is valid"),
            panel,
        }
    }
}
impl Plugin for GDesignPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
    fn panel(&self) -> &Panel {
        &self.panel
    }
    fn invoke(&mut self, action: &str) -> Result<(), PluginError> {
        match action {
            "text" => Ok(()),
            _ => Err(PluginError::UnknownAction(action.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use cakewalk::{ShapeKind, ShapeRecipe, TextRecipe};
    #[test]
    fn design_plugin_uses_shared_recipes() {
        assert!(ShapeRecipe::new(ShapeKind::Ellipse, Some("#fff".into()), None, 0.0).is_ok());
        assert!(TextRecipe::new("Heading", "Sans Serif", 24.0, "#000").is_ok());
    }
}
