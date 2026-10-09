use components::Panel;
use stonecutter::{Plugin, PluginError, PluginId, PluginManifest, Version};

pub const PYTHON_BRIDGE: stonecutter::PythonBridge =
    stonecutter::python_bridge!("pinwheel", stonecutter::requirements_txt![],);

pub struct PinwheelPlugin {
    manifest: PluginManifest,
    panel: Panel,
}

impl Default for PinwheelPlugin {
    fn default() -> Self {
        let panel = Panel::new("Pinwheel", |ui| {
            ui.row(|ui| {
                ui.button("Add reference", "add_reference");
                ui.button("Pop out", "pop_out");
            });
        })
        .expect("static panel is valid");
        Self {
            manifest: PluginManifest::new(
                PluginId::new("cc.hellings.pinwheel").expect("static ID"),
                "Pinwheel",
                Version::new(0, 1, 0),
                Version::new(5, 2, 0),
            )
            .expect("static manifest"),
            panel,
        }
    }
}
impl Plugin for PinwheelPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
    fn panel(&self) -> &Panel {
        &self.panel
    }
    fn invoke(&mut self, action: &str) -> Result<(), PluginError> {
        match action {
            "add_reference" | "pop_out" => Ok(()),
            _ => Err(PluginError::UnknownAction(action.into())),
        }
    }
}
