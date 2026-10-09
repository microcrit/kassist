use components::Panel;
use stonecutter::{Plugin, PluginError, PluginId, PluginManifest, Version};

pub const PYTHON_BRIDGE: stonecutter::PythonBridge =
    stonecutter::python_bridge!("cakewalk_modifier", stonecutter::requirements_txt![],);

stonecutter::export_python_bridge_version!(kassist_cakewalk_modifier_bridge_version);

pub struct CakewalkPlugin {
    manifest: PluginManifest,
    panel: Panel,
}

impl Default for CakewalkPlugin {
    fn default() -> Self {
        let panel = Panel::new("Cakewalk Modifiers", |ui| {
            ui.button("Apply hair guides", "hair-guides");
            ui.button("Clear hair guides", "clear-hair-guides");
        })
        .expect("static panel is valid");
        Self {
            manifest: PluginManifest::new(
                PluginId::new("cc.hellings.cakewalk").expect("static id is valid"),
                "Cakewalk Modifiers",
                Version::new(0, 1, 0),
                Version::new(5, 2, 0),
            )
            .expect("static manifest is valid"),
            panel,
        }
    }
}

impl Plugin for CakewalkPlugin {
    fn manifest(&self) -> &PluginManifest { &self.manifest }
    fn panel(&self) -> &Panel { &self.panel }
    fn invoke(&mut self, action: &str) -> Result<(), PluginError> {
        match action {
            "hair-guides" | "clear-hair-guides" => Ok(()),
            _ => Err(PluginError::UnknownAction(action.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CakewalkPlugin;
    use stonecutter::Plugin;

    #[test]
    fn modifier_panel_exposes_hair_actions() {
        let plugin = CakewalkPlugin::default();
        assert!(plugin.panel().actions().contains(&"hair-guides".to_owned()));
    }
}
