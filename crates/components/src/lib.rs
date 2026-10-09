use commonext::{ensure, ValidationError};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    id: String,
    kind: ComponentKind,
}

impl Component {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn kind(&self) -> &ComponentKind {
        &self.kind
    }

    fn visit(&self, visitor: &mut impl FnMut(&Component)) {
        visitor(self);
        match &self.kind {
            ComponentKind::Group { children, .. } | ComponentKind::Scroll { children } => {
                for child in children {
                    child.visit(visitor);
                }
            }
            ComponentKind::Tabs { tabs } => {
                for tab in tabs {
                    for child in &tab.components {
                        child.visit(visitor);
                    }
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ComponentKind {
    Text(String),

    Button {
        label: String,
        action: String,
    },

    Toggle {
        label: String,
        checked: bool,
        action: String,
    },

    TextInput {
        label: String,
        value: String,
        placeholder: String,
        action: String,
    },

    TextArea {
        label: String,
        value: String,
        placeholder: String,
        rows: u8,
        action: String,
    },

    Select {
        label: String,
        options: Vec<String>,
        selected: usize,
        action: String,
    },

    RadioGroup {
        label: String,
        options: Vec<String>,
        selected: usize,
        action: String,
    },

    Number {
        label: String,
        value: f64,
        minimum: f64,
        maximum: f64,
        step: f64,
        decimals: u8,
        action: String,
    },

    Slider {
        label: String,
        value: f64,
        minimum: f64,
        maximum: f64,
        decimals: u8,
        action: String,
    },

    ColorField {
        label: String,
        value: String,
        action: String,
    },

    FilePicker {
        label: String,
        value: String,
        caption: String,
        mode: FileMode,
        action: String,
    },

    Progress {
        label: String,
        value: f64,
    },

    Link {
        label: String,
        url: String,
    },

    Group {
        direction: Direction,
        children: Vec<Component>,
    },

    Tabs {
        tabs: Vec<Tab>,
    },

    Scroll {
        children: Vec<Component>,
    },

    Separator,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    title: String,
    components: Vec<Component>,
}

impl Tab {
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn components(&self) -> &[Component] {
        &self.components
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileMode {
    Open,
    Save,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    title: String,
    components: Vec<Component>,
}

impl Panel {
    pub fn new(
        title: impl Into<String>,
        build: impl FnOnce(&mut Ui),
    ) -> Result<Self, ValidationError> {
        let title = title.into();
        ensure!(!title.trim().is_empty(), "panel_title", "must not be empty");
        let mut ui = Ui::root();
        build(&mut ui);
        let panel = Self {
            title,
            components: ui.components,
        };
        panel.validate()?;
        Ok(panel)
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn components(&self) -> &[Component] {
        &self.components
    }

    pub fn actions(&self) -> Vec<String> {
        let mut actions = Vec::new();
        for component in &self.components {
            component.visit(&mut |component| match component.kind() {
                ComponentKind::Button { action, .. }
                | ComponentKind::Toggle { action, .. }
                | ComponentKind::TextInput { action, .. }
                | ComponentKind::TextArea { action, .. }
                | ComponentKind::Select { action, .. }
                | ComponentKind::RadioGroup { action, .. }
                | ComponentKind::Number { action, .. }
                | ComponentKind::Slider { action, .. }
                | ComponentKind::ColorField { action, .. }
                | ComponentKind::FilePicker { action, .. } => actions.push(action.clone()),
                _ => {}
            });
        }
        actions
    }

    fn validate(&self) -> Result<(), ValidationError> {
        let mut ids = BTreeSet::new();
        let mut error = None;
        for component in &self.components {
            component.visit(&mut |component| {
                if !ids.insert(component.id().to_owned()) {
                    error.get_or_insert_with(|| {
                        ValidationError::new(
                            "component_id",
                            format!("'{}' is used more than once", component.id()),
                        )
                    });
                    return;
                }
                if let Some(problem) = problem(component) {
                    error.get_or_insert(problem);
                }
            });
        }
        error.map_or(Ok(()), Err)
    }
}

fn problem(component: &Component) -> Option<ValidationError> {
    match component.kind() {
        ComponentKind::Button { label, action }
        | ComponentKind::Toggle { label, action, .. }
        | ComponentKind::TextInput { label, action, .. } => {
            if label.trim().is_empty() || action.trim().is_empty() {
                return Some(ValidationError::new(
                    "control",
                    "label and action must not be empty",
                ));
            }
            None
        }
        ComponentKind::TextArea {
            label,
            action,
            rows,
            ..
        } => {
            if label.trim().is_empty() || action.trim().is_empty() {
                return Some(ValidationError::new(
                    "text_area",
                    "label and action must not be empty",
                ));
            } else if *rows == 0 {
                return Some(ValidationError::new(
                    "text_area",
                    "must reserve at least one row",
                ));
            }
            None
        }
        ComponentKind::Select {
            label,
            options,
            selected,
            action,
        } => {
            if label.trim().is_empty() || action.trim().is_empty() {
                return Some(ValidationError::new(
                    "select",
                    "label and action must not be empty",
                ));
            } else if options.is_empty()
                || options.iter().any(|option| option.trim().is_empty())
                || *selected >= options.len()
            {
                return Some(ValidationError::new(
                    "select",
                    "must have non-empty options and a valid selection",
                ));
            }
            None
        }
        ComponentKind::RadioGroup {
            label,
            options,
            selected,
            action,
        } => {
            if label.trim().is_empty() || action.trim().is_empty() {
                return Some(ValidationError::new(
                    "radio_group",
                    "label and action must not be empty",
                ));
            } else if options.is_empty()
                || options.iter().any(|option| option.trim().is_empty())
                || *selected >= options.len()
            {
                return Some(ValidationError::new(
                    "radio_group",
                    "must have non-empty options and a valid selection",
                ));
            }
            None
        }
        ComponentKind::Number {
            label,
            value,
            minimum,
            maximum,
            step,
            action,
            ..
        } => {
            if label.trim().is_empty() || action.trim().is_empty() {
                return Some(ValidationError::new(
                    "number",
                    "label and action must not be empty",
                ));
            } else if !value.is_finite()
                || !minimum.is_finite()
                || !maximum.is_finite()
                || !step.is_finite()
                || *minimum > *maximum
                || *value < *minimum
                || *value > *maximum
                || *step <= 0.0
            {
                return Some(ValidationError::new(
                    "number",
                    "must have a finite in-range value and a positive step",
                ));
            }
            None
        }
        ComponentKind::Slider {
            label,
            value,
            minimum,
            maximum,
            action,
            ..
        } => {
            if label.trim().is_empty() || action.trim().is_empty() {
                return Some(ValidationError::new(
                    "slider",
                    "label and action must not be empty",
                ));
            } else if !value.is_finite()
                || !minimum.is_finite()
                || !maximum.is_finite()
                || *minimum >= *maximum
                || *value < *minimum
                || *value > *maximum
            {
                return Some(ValidationError::new(
                    "slider",
                    "must have a finite in-range value inside a non-empty range",
                ));
            }
            None
        }
        ComponentKind::ColorField {
            label,
            value,
            action,
        } => {
            if label.trim().is_empty() || action.trim().is_empty() {
                return Some(ValidationError::new(
                    "color",
                    "label and action must not be empty",
                ));
            } else if !is_color(value) {
                return Some(ValidationError::new(
                    "color",
                    "must be six or eight hexadecimal digits",
                ));
            }
            None
        }
        ComponentKind::FilePicker {
            label,
            caption,
            action,
            ..
        } => {
            if label.trim().is_empty() || caption.trim().is_empty() || action.trim().is_empty() {
                return Some(ValidationError::new(
                    "file",
                    "label, caption and action must not be empty",
                ));
            }
            None
        }
        ComponentKind::Progress { value, .. } => {
            if !value.is_finite() || !(0.0..=1.0).contains(value) {
                return Some(ValidationError::new(
                    "progress",
                    "must be a fraction between 0 and 1",
                ));
            }
            None
        }
        ComponentKind::Link { label, url } => {
            if label.trim().is_empty() || url.trim().is_empty() {
                return Some(ValidationError::new(
                    "link",
                    "label and url must not be empty",
                ));
            } else if !url.contains("://") {
                return Some(ValidationError::new(
                    "link",
                    "url must include a scheme such as https://",
                ));
            }
            None
        }
        ComponentKind::Tabs { tabs } => {
            if tabs.is_empty() || tabs.iter().any(|tab| tab.title.trim().is_empty()) {
                return Some(ValidationError::new("tabs", "need a title for every tab"));
            }
            None
        }
        _ => None,
    }
}

pub struct Ui {
    components: Vec<Component>,
    prefix: String,
    next_id: usize,
}

impl Ui {
    fn root() -> Self {
        Self {
            components: Vec::new(),
            prefix: "component".into(),
            next_id: 0,
        }
    }

    pub fn text(&mut self, text: impl Into<String>) -> &mut Self {
        self.push(ComponentKind::Text(text.into()));
        self
    }

    pub fn button(&mut self, label: impl Into<String>, action: impl Into<String>) -> &mut Self {
        self.push(ComponentKind::Button {
            label: label.into(),
            action: action.into(),
        });
        self
    }

    pub fn toggle(
        &mut self,
        label: impl Into<String>,
        checked: bool,
        action: impl Into<String>,
    ) -> &mut Self {
        self.push(ComponentKind::Toggle {
            label: label.into(),
            checked,
            action: action.into(),
        });
        self
    }

    pub fn text_input(
        &mut self,
        label: impl Into<String>,
        value: impl Into<String>,
        placeholder: impl Into<String>,
        action: impl Into<String>,
    ) -> &mut Self {
        self.push(ComponentKind::TextInput {
            label: label.into(),
            value: value.into(),
            placeholder: placeholder.into(),
            action: action.into(),
        });
        self
    }

    pub fn text_area(
        &mut self,
        label: impl Into<String>,
        value: impl Into<String>,
        placeholder: impl Into<String>,
        rows: u8,
        action: impl Into<String>,
    ) -> &mut Self {
        self.push(ComponentKind::TextArea {
            label: label.into(),
            value: value.into(),
            placeholder: placeholder.into(),
            rows,
            action: action.into(),
        });
        self
    }

    pub fn select<I, S>(
        &mut self,
        label: impl Into<String>,
        options: I,
        selected: usize,
        action: impl Into<String>,
    ) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.push(ComponentKind::Select {
            label: label.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected,
            action: action.into(),
        });
        self
    }

    pub fn radio_group<I, S>(
        &mut self,
        label: impl Into<String>,
        options: I,
        selected: usize,
        action: impl Into<String>,
    ) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.push(ComponentKind::RadioGroup {
            label: label.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected,
            action: action.into(),
        });
        self
    }

    pub fn slider(
        &mut self,
        label: impl Into<String>,
        value: f64,
        minimum: f64,
        maximum: f64,
        decimals: u8,
        action: impl Into<String>,
    ) -> &mut Self {
        self.push(ComponentKind::Slider {
            label: label.into(),
            value,
            minimum,
            maximum,
            decimals,
            action: action.into(),
        });
        self
    }

    pub fn color_field(
        &mut self,
        label: impl Into<String>,
        value: impl Into<String>,
        action: impl Into<String>,
    ) -> &mut Self {
        self.push(ComponentKind::ColorField {
            label: label.into(),
            value: value.into(),
            action: action.into(),
        });
        self
    }

    pub fn file_picker(
        &mut self,
        label: impl Into<String>,
        value: impl Into<String>,
        caption: impl Into<String>,
        mode: FileMode,
        action: impl Into<String>,
    ) -> &mut Self {
        self.push(ComponentKind::FilePicker {
            label: label.into(),
            value: value.into(),
            caption: caption.into(),
            mode,
            action: action.into(),
        });
        self
    }

    pub fn progress(&mut self, label: impl Into<String>, value: f64) -> &mut Self {
        self.push(ComponentKind::Progress {
            label: label.into(),
            value,
        });
        self
    }

    pub fn link(&mut self, label: impl Into<String>, url: impl Into<String>) -> &mut Self {
        self.push(ComponentKind::Link {
            label: label.into(),
            url: url.into(),
        });
        self
    }

    pub fn number(
        &mut self,
        label: impl Into<String>,
        value: f64,
        minimum: f64,
        maximum: f64,
        step: f64,
        decimals: u8,
        action: impl Into<String>,
    ) -> &mut Self {
        self.push(ComponentKind::Number {
            label: label.into(),
            value,
            minimum,
            maximum,
            step,
            decimals,
            action: action.into(),
        });
        self
    }

    pub fn separator(&mut self) -> &mut Self {
        self.push(ComponentKind::Separator);
        self
    }

    pub fn row(&mut self, build: impl FnOnce(&mut Ui)) -> &mut Self {
        self.group(Direction::Horizontal, build)
    }

    pub fn column(&mut self, build: impl FnOnce(&mut Ui)) -> &mut Self {
        self.group(Direction::Vertical, build)
    }

    pub fn scroll(&mut self, build: impl FnOnce(&mut Ui)) -> &mut Self {
        let id = self.next_component_id();
        let mut child = self.nested(id.clone());
        build(&mut child);
        self.components.push(Component {
            id,
            kind: ComponentKind::Scroll {
                children: child.components,
            },
        });
        self
    }

    pub fn tabs(&mut self, build: impl FnOnce(&mut TabBar)) -> &mut Self {
        let id = self.next_component_id();
        let mut bar = TabBar {
            tabs: Vec::new(),
            prefix: id.clone(),
            next_id: 0,
        };
        build(&mut bar);
        self.components.push(Component {
            id,
            kind: ComponentKind::Tabs { tabs: bar.tabs },
        });
        self
    }

    fn group(&mut self, direction: Direction, build: impl FnOnce(&mut Ui)) -> &mut Self {
        let id = self.next_component_id();
        let mut child = self.nested(id.clone());
        build(&mut child);
        self.components.push(Component {
            id,
            kind: ComponentKind::Group {
                direction,
                children: child.components,
            },
        });
        self
    }

    fn nested(&self, prefix: String) -> Self {
        Self {
            components: Vec::new(),
            prefix,
            next_id: 0,
        }
    }

    fn push(&mut self, kind: ComponentKind) {
        let id = self.next_component_id();
        self.components.push(Component { id, kind });
    }
    fn next_component_id(&mut self) -> String {
        let id = format!("{}-{}", self.prefix, self.next_id);
        self.next_id += 1;
        id
    }
}

pub struct TabBar {
    tabs: Vec<Tab>,
    prefix: String,
    next_id: usize,
}

impl TabBar {
    pub fn tab(&mut self, title: impl Into<String>, build: impl FnOnce(&mut Ui)) -> &mut Self {
        let prefix = format!("{}-{}", self.prefix, self.next_id);
        self.next_id += 1;
        let mut ui = Ui::root();
        ui.prefix = prefix;
        build(&mut ui);
        self.tabs.push(Tab {
            title: title.into(),
            components: ui.components,
        });
        self
    }
}

fn is_color(value: &str) -> bool {
    let digits = value.strip_prefix('#').unwrap_or(value);
    matches!(digits.len(), 6 | 8)
        && digits
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f' | b'A'..=b'F'))
}

#[cfg(test)]
mod tests {
    use super::{ComponentKind, Direction, FileMode, Panel};

    #[test]
    fn panel_builder_generates_a_small_component_tree() {
        let panel = Panel::new("Tools", |ui| {
            ui.row(|ui| {
                ui.button("Reset", "reset");
                ui.button("Hide", "hide");
            });
        })
        .unwrap();
        assert_eq!(panel.title(), "Tools");
        assert_eq!(panel.actions(), ["reset", "hide"]);
        assert!(matches!(
            panel.components()[0].kind(),
            ComponentKind::Group {
                direction: Direction::Horizontal,
                ..
            }
        ));
    }

    #[test]
    fn blank_button_actions_are_rejected() {
        let error = Panel::new("Tools", |ui| {
            ui.button("Reset", " ");
        })
        .unwrap_err();
        assert_eq!(error.field(), "control");
    }

    #[test]
    fn stateful_controls_contribute_actions_and_validate_selection() {
        let panel = Panel::new("Tools", |ui| {
            ui.toggle("Live", true, "set_live");
            ui.text_input("Tag", "faces", "tag", "set_tag");
            ui.select("Mode", ["Fast", "Accurate"], 1, "set_mode");
            ui.number("Copies", 4.0, 0.0, 24.0, 1.0, 0, "set_copies");
        })
        .unwrap();
        assert_eq!(
            panel.actions(),
            ["set_live", "set_tag", "set_mode", "set_copies"]
        );

        let error = Panel::new("Tools", |ui| {
            ui.select("Mode", ["Fast"], 2, "set_mode");
        })
        .unwrap_err();
        assert_eq!(error.field(), "select");
        assert!(matches!(
            panel.components()[0].kind(),
            ComponentKind::Toggle { .. }
        ));

        let error = Panel::new("Tools", |ui| {
            ui.number("Copies", 1.0, 2.0, 24.0, 1.0, 0, "set_copies");
        })
        .unwrap_err();
        assert_eq!(error.field(), "number");
    }

    #[test]
    fn plugin_controls_cover_files_colors_and_ranges() {
        let panel = Panel::new("Export", |ui| {
            ui.text_area(
                "Prompt",
                "a studio interior",
                "describe the scene",
                4,
                "set_prompt",
            );
            ui.radio_group("Quality", ["Draft", "Final"], 1, "set_quality");
            ui.slider("Denoise", 0.5, 0.0, 1.0, 2, "set_denoise");
            ui.color_field("Tint", "#ff8800cc", "set_tint");
            ui.file_picker(
                "Output",
                "/tmp/out.png",
                "Save the render",
                FileMode::Save,
                "set_output",
            );
            ui.progress("Rendering", 0.25);
            ui.link("Docs", "https://docs.krita.org");
        })
        .unwrap();
        assert_eq!(
            panel.actions(),
            [
                "set_prompt",
                "set_quality",
                "set_denoise",
                "set_tint",
                "set_output"
            ]
        );
        assert!(matches!(
            panel.components()[1].kind(),
            ComponentKind::RadioGroup { selected: 1, .. }
        ));
        assert!(matches!(
            panel.components()[4].kind(),
            ComponentKind::FilePicker {
                mode: FileMode::Save,
                ..
            }
        ));
        assert!(matches!(
            panel.components()[6].kind(),
            ComponentKind::Link { .. }
        ));
    }

    #[test]
    fn nested_controls_are_validated_and_collected_from_every_container() {
        let panel = Panel::new("Pipeline", |ui| {
            ui.scroll(|ui| {
                ui.button("Run", "run");
            });
            ui.tabs(|tabs| {
                tabs.tab("Model", |ui| {
                    ui.select("Sampler", ["Euler", "DPM"], 0, "set_sampler");
                })
                .tab("Paths", |ui| {
                    ui.text_input("Weights", "", "path", "set_weights");
                });
            });
        })
        .unwrap();
        assert_eq!(panel.actions(), ["run", "set_sampler", "set_weights"]);
        let ComponentKind::Tabs { tabs } = panel.components()[1].kind() else {
            panic!("expected a tabbed group");
        };
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[1].title(), "Paths");

        let error = Panel::new("Pipeline", |ui| {
            ui.tabs(|tabs| {
                tabs.tab(" ", |ui| {
                    ui.button("Run", " ");
                });
            });
        })
        .unwrap_err();
        assert_eq!(error.field(), "tabs");
    }

    #[test]
    fn invalid_plugin_control_values_name_their_field() {
        fn rejects(field: &str, build: impl Fn(&mut super::Ui)) {
            let error = Panel::new("Pipeline", |ui| build(ui)).unwrap_err();
            assert_eq!(error.field(), field);
        }

        rejects("text_area", |ui| {
            ui.text_area("Prompt", "", "", 0, "set_prompt");
        });
        rejects("radio_group", |ui| {
            ui.radio_group("Quality", ["Draft"], 3, "set_quality");
        });
        rejects("slider", |ui| {
            ui.slider("Denoise", 0.5, 0.5, 0.5, 2, "set_denoise");
        });
        rejects("color", |ui| {
            ui.color_field("Tint", "not-a-color", "set_tint");
        });
        rejects("file", |ui| {
            ui.file_picker("Output", "", " ", FileMode::Open, "set_output");
        });
        rejects("progress", |ui| {
            ui.progress("Rendering", 1.5);
        });
        rejects("link", |ui| {
            ui.link("Docs", "krita.org");
        });
    }
}
