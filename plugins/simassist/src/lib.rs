use components::Panel;
use stonecutter::{Plugin, PluginError, PluginId, PluginManifest, Version};

pub const PYTHON_BRIDGE: stonecutter::PythonBridge =
    stonecutter::python_bridge!("simassist", stonecutter::requirements_txt![],);

stonecutter::export_python_bridge_version!(kassist_simassist_bridge_version);

#[unsafe(no_mangle)]
pub extern "C" fn kassist_simassist_mirror_vertical(
    x: f32,
    y: f32,
    axis_x: f32,
    output_x: *mut f32,
    output_y: *mut f32,
) -> bool {
    if output_x.is_null()
        || output_y.is_null()
        || !x.is_finite()
        || !y.is_finite()
        || !axis_x.is_finite()
    {
        return false;
    }
    unsafe {
        *output_x = 2.0 * axis_x - x;
        *output_y = y;
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn kassist_simassist_mirror_line(
    x: f32,
    y: f32,
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
    output_x: *mut f32,
    output_y: *mut f32,
) -> bool {
    if output_x.is_null()
        || output_y.is_null()
        || ![x, y, start_x, start_y, end_x, end_y]
            .into_iter()
            .all(f32::is_finite)
    {
        return false;
    }
    let delta_x = end_x - start_x;
    let delta_y = end_y - start_y;
    let length_squared = delta_x * delta_x + delta_y * delta_y;
    if length_squared <= f32::EPSILON {
        return false;
    }
    let projection = ((x - start_x) * delta_x + (y - start_y) * delta_y) / length_squared;
    unsafe {
        *output_x = 2.0 * (start_x + projection * delta_x) - x;
        *output_y = 2.0 * (start_y + projection * delta_y) - y;
    }
    true
}

const MAX_AXES: usize = 8;
const MAX_RADIAL_SEGMENTS: u8 = 24;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SymmetryAxis {
    Vertical { x: f32 },
    Horizontal { y: f32 },
    Line { start: Point, end: Point },
}

impl SymmetryAxis {
    pub fn mirror(self, point: Point) -> Point {
        match self {
            Self::Vertical { x } => Point::new(2.0 * x - point.x, point.y),
            Self::Horizontal { y } => Point::new(point.x, 2.0 * y - point.y),
            Self::Line { start, end } => {
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                let length_squared = dx * dx + dy * dy;
                if length_squared <= f32::EPSILON {
                    return point;
                }
                let projection =
                    ((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared;
                let closest = Point::new(start.x + projection * dx, start.y + projection * dy);
                Point::new(2.0 * closest.x - point.x, 2.0 * closest.y - point.y)
            }
        }
    }

    pub fn drag_to(&mut self, point: Point) {
        match self {
            Self::Vertical { x } => *x = point.x,
            Self::Horizontal { y } => *y = point.y,
            Self::Line { start, end } => {
                let to_start = squared_distance(*start, point);
                let to_end = squared_distance(*end, point);
                if to_start <= to_end {
                    *start = point;
                } else {
                    *end = point;
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnglePicker {
    center: Point,
    angle_radians: f32,
}

impl AnglePicker {
    pub const fn new(center: Point, angle_radians: f32) -> Self {
        Self {
            center,
            angle_radians,
        }
    }
    pub fn center(&self) -> Point {
        self.center
    }
    pub fn angle_radians(&self) -> f32 {
        self.angle_radians
    }
    pub fn drag_to(&mut self, point: Point) {
        self.angle_radians = (point.y - self.center.y).atan2(point.x - self.center.x);
    }
    pub fn handle(&self, radius: f32) -> Point {
        Point::new(
            self.center.x + radius * self.angle_radians.cos(),
            self.center.y + radius * self.angle_radians.sin(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadialSymmetry {
    picker: AnglePicker,
    segments: u8,
}

impl RadialSymmetry {
    pub fn new(picker: AnglePicker, segments: u8) -> Result<Self, SymmetryError> {
        if !(2..=MAX_RADIAL_SEGMENTS).contains(&segments) {
            return Err(SymmetryError::InvalidSegmentCount);
        }
        Ok(Self { picker, segments })
    }
    pub fn picker(&self) -> AnglePicker {
        self.picker
    }
    pub fn set_picker(&mut self, picker: AnglePicker) {
        self.picker = picker;
    }
    pub fn segments(&self) -> u8 {
        self.segments
    }

    pub fn copies(&self, point: Point) -> Vec<Point> {
        let mut copies = Vec::with_capacity(usize::from(self.segments) * 2);
        let local_x = point.x - self.picker.center.x;
        let local_y = point.y - self.picker.center.y;
        let step = std::f32::consts::TAU / f32::from(self.segments);
        for index in 0..self.segments {
            let angle = self.picker.angle_radians + f32::from(index) * step;
            copies.push(rotate_local(self.picker.center, local_x, local_y, angle));
            copies.push(rotate_local(self.picker.center, local_x, -local_y, angle));
        }
        copies
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SymmetryModel {
    axes: Vec<SymmetryAxis>,
    radial: Option<RadialSymmetry>,
}

impl Default for SymmetryModel {
    fn default() -> Self {
        Self {
            axes: vec![SymmetryAxis::Vertical { x: 0.0 }],
            radial: None,
        }
    }
}

impl SymmetryModel {
    pub fn axes(&self) -> &[SymmetryAxis] {
        &self.axes
    }
    pub fn radial(&self) -> Option<RadialSymmetry> {
        self.radial
    }
    pub fn add_axis(&mut self, axis: SymmetryAxis) -> Result<(), SymmetryError> {
        if self.axes.len() == MAX_AXES {
            return Err(SymmetryError::TooManyAxes);
        }
        if !axis_is_finite(axis) {
            return Err(SymmetryError::NonFinitePoint);
        }
        self.axes.push(axis);
        Ok(())
    }
    pub fn set_radial(&mut self, radial: Option<RadialSymmetry>) {
        self.radial = radial;
    }

    pub fn copies(&self, point: Point) -> Vec<Point> {
        let mut points = vec![point];
        for axis in &self.axes {
            let reflected: Vec<_> = points.iter().map(|point| axis.mirror(*point)).collect();
            points.extend(reflected);
        }
        if let Some(radial) = self.radial {
            points
                .into_iter()
                .flat_map(|point| radial.copies(point))
                .collect()
        } else {
            points
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerspectiveGuide {
    vanishing_point: Point,
    strength: f32,
}

impl PerspectiveGuide {
    pub fn from_axes(
        first: SymmetryAxis,
        second: SymmetryAxis,
        strength: f32,
    ) -> Result<Self, SymmetryError> {
        let (a, b) = as_line(first).ok_or(SymmetryError::NeedFreeAngleAxis)?;
        let (c, d) = as_line(second).ok_or(SymmetryError::NeedFreeAngleAxis)?;
        let denominator = cross(subtract(b, a), subtract(d, c));
        if denominator.abs() <= f32::EPSILON {
            return Err(SymmetryError::ParallelAxes);
        }
        let factor = cross(subtract(c, a), subtract(d, c)) / denominator;
        Ok(Self {
            vanishing_point: add(a, scale(subtract(b, a), factor)),
            strength: strength.clamp(0.0, 1.0),
        })
    }
    pub fn vanishing_point(&self) -> Point {
        self.vanishing_point
    }

    pub fn warp(&self, point: Point) -> Point {
        Point::new(
            point.x + (self.vanishing_point.x - point.x) * self.strength,
            point.y + (self.vanishing_point.y - point.y) * self.strength,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymmetryError {
    TooManyAxes,
    InvalidSegmentCount,
    NonFinitePoint,
    NeedFreeAngleAxis,
    ParallelAxes,
}

pub struct SimAssistPlugin {
    manifest: PluginManifest,
    panel: Panel,
    model: SymmetryModel,
    visible: bool,
}

impl Default for SimAssistPlugin {
    fn default() -> Self {
        let panel = Panel::new("Symmetry", |ui| {
            ui.row(|ui| {
                ui.button("Show / hide", "toggle_visible");
                ui.button("Reset guides", "reset_guides");
            });
        })
        .expect("static panel is valid");
        Self {
            manifest: PluginManifest::new(
                PluginId::new("cc.hellings.simassist").expect("static ID is valid"),
                "Symmetry Reference",
                Version::new(0, 2, 0),
                Version::new(5, 2, 0),
            )
            .expect("static manifest is valid"),
            panel,
            model: SymmetryModel::default(),
            visible: true,
        }
    }
}

impl SimAssistPlugin {
    pub fn model(&self) -> &SymmetryModel {
        &self.model
    }
    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

impl Plugin for SimAssistPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
    fn panel(&self) -> &Panel {
        &self.panel
    }
    fn invoke(&mut self, action: &str) -> Result<(), PluginError> {
        match action {
            "toggle_visible" => {
                self.visible = !self.visible;
                Ok(())
            }
            "reset_guides" => {
                self.model = SymmetryModel::default();
                Ok(())
            }
            _ => Err(PluginError::UnknownAction(action.into())),
        }
    }
}

fn rotate_local(center: Point, x: f32, y: f32, angle: f32) -> Point {
    Point::new(
        center.x + x * angle.cos() - y * angle.sin(),
        center.y + x * angle.sin() + y * angle.cos(),
    )
}
fn squared_distance(left: Point, right: Point) -> f32 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    dx * dx + dy * dy
}
fn axis_is_finite(axis: SymmetryAxis) -> bool {
    match axis {
        SymmetryAxis::Vertical { x } => x.is_finite(),
        SymmetryAxis::Horizontal { y } => y.is_finite(),
        SymmetryAxis::Line { start, end } => start.is_finite() && end.is_finite(),
    }
}
fn as_line(axis: SymmetryAxis) -> Option<(Point, Point)> {
    if let SymmetryAxis::Line { start, end } = axis {
        Some((start, end))
    } else {
        None
    }
}
fn subtract(left: Point, right: Point) -> Point {
    Point::new(left.x - right.x, left.y - right.y)
}
fn add(left: Point, right: Point) -> Point {
    Point::new(left.x + right.x, left.y + right.y)
}
fn scale(point: Point, factor: f32) -> Point {
    Point::new(point.x * factor, point.y * factor)
}
fn cross(left: Point, right: Point) -> f32 {
    left.x * right.y - left.y * right.x
}

#[cfg(test)]
mod tests {
    use super::{
        AnglePicker, PerspectiveGuide, Point, RadialSymmetry, SymmetryAxis, SymmetryModel,
    };
    #[test]
    fn mirrors_across_a_vertical_axis() {
        assert_eq!(
            SymmetryAxis::Vertical { x: 10.0 }.mirror(Point::new(13.0, 7.0)),
            Point::new(7.0, 7.0)
        );
    }
    #[test]
    fn native_line_bridge_mirrors_across_a_horizontal_axis() {
        let mut x = 0.0;
        let mut y = 0.0;
        assert!(super::kassist_simassist_mirror_line(
            3.0, 7.0, 0.0, 2.0, 1.0, 2.0, &mut x, &mut y,
        ));
        assert_eq!(Point::new(x, y), Point::new(3.0, -3.0));
    }
    #[test]
    fn picker_tracks_dragged_angle() {
        let mut picker = AnglePicker::new(Point::new(0.0, 0.0), 0.0);
        picker.drag_to(Point::new(0.0, 2.0));
        assert!((picker.angle_radians() - std::f32::consts::FRAC_PI_2).abs() < 0.001);
    }
    #[test]
    fn radial_symmetry_generates_two_copies_per_segment() {
        let radial = RadialSymmetry::new(AnglePicker::new(Point::new(0.0, 0.0), 0.0), 4).unwrap();
        assert_eq!(radial.copies(Point::new(2.0, 1.0)).len(), 8);
    }
    #[test]
    fn model_combines_multiple_axes() {
        let mut model = SymmetryModel::default();
        model.add_axis(SymmetryAxis::Horizontal { y: 0.0 }).unwrap();
        assert_eq!(model.copies(Point::new(2.0, 3.0)).len(), 4);
    }
    #[test]
    fn estimates_vanishing_point_from_axes() {
        let guide = PerspectiveGuide::from_axes(
            SymmetryAxis::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(2.0, 2.0),
            },
            SymmetryAxis::Line {
                start: Point::new(0.0, 2.0),
                end: Point::new(2.0, 0.0),
            },
            0.5,
        )
        .unwrap();
        assert_eq!(guide.vanishing_point(), Point::new(1.0, 1.0));
        assert_eq!(guide.warp(Point::new(3.0, 1.0)), Point::new(2.0, 1.0));
    }
}
