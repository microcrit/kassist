"""Cakewalk modifier and manual-brush docker."""

from krita import DockWidget, DockWidgetFactory, DockWidgetFactoryBase, Krita
from PyQt5.QtWidgets import QHBoxLayout, QLabel, QPushButton, QTabWidget, QVBoxLayout, QWidget

from . import kassist_krita as krita
from .kassist_cakewalk import PixelContext, PixelSurface, ToolPanel
from .kassist_cakewalk_plugs import listed_tools, load, user_directory, write_template
from .kassist_components import Panel

HAIR_GUIDE_LAYER = "Cakewalk Hair Guides"


class CakewalkDocker(DockWidget):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Cakewalk Modifiers")
        self.settings = {
            "density": 1.0,
            "length": 42.0,
            "direction": -90.0,
            "gravity": 0.55,
            "opacity": 104.0,
            "color": "#d5a66aff",
        }
        content, layout = QWidget(self), QVBoxLayout()
        content.setLayout(layout)
        pages = QTabWidget(content)
        layout.addWidget(pages)
        modifier_page, modifier_layout = QWidget(pages), QVBoxLayout()
        modifier_page.setLayout(modifier_layout)
        self.status = QLabel("Select painted artwork, then apply a Cakewalk modifier.")
        modifier_layout.addWidget(self.status)
        Panel("Hair modifier", self._dispatch).number(
            "Density", 1.0, 0.1, 4.0, 0.1, 1, "density"
        ).number("Guide length", 42.0, 2.0, 300.0, 1.0, 0, "length").slider(
            "Direction", -90.0, -180.0, 180.0, 0, "direction"
        ).slider(
            "Gravity", 0.55, 0.0, 1.0, 2, "gravity"
        ).color_field(
            "Guide color", "#d5a66a", "color"
        ).slider(
            "Layer opacity", 104.0, 16.0, 220.0, 0, "opacity"
        ).row(
            lambda ui: ui.button("Apply hair guides", "apply").button(
                "Clear hair guides", "clear"
            )
        ).mount(modifier_layout)
        pages.addTab(modifier_page, "Modifiers")

        self.brush_page, self.brush_layout = QWidget(pages), QVBoxLayout()
        self.brush_page.setLayout(self.brush_layout)
        self.brush_status = QLabel()
        self.brush_status.setWordWrap(True)
        controls, controls_layout = QWidget(self.brush_page), QHBoxLayout()
        controls_layout.setContentsMargins(0, 0, 0, 0)
        controls.setLayout(controls_layout)
        reload_button = QPushButton("Reload brushes", controls)
        template_button = QPushButton("Create user brush", controls)
        reload_button.clicked.connect(self.reload_brushes)
        template_button.clicked.connect(self.create_user_brush)
        controls_layout.addWidget(reload_button)
        controls_layout.addWidget(template_button)
        self.brush_layout.addWidget(controls)
        self.brush_layout.addWidget(self.brush_status)
        pages.addTab(self.brush_page, "Brushes")
        self.tool_panel = None
        self.reload_brushes()
        self.setWidget(content)

    def canvasChanged(self, canvas):
        self.canvas = canvas
        if canvas is None and self.tool_panel is not None:
            self.tool_panel.disable_canvas_tool()
            self.brush_status.setText("Open a document before enabling a Cakewalk canvas brush.")

    def _dispatch(self, action, value):
        if action in self.settings:
            self.settings[action] = value
        elif action == "apply":
            self.apply_hair()
        elif action == "clear":
            self.clear_hair()
        return True

    def apply_hair(self):
        document = Krita.instance().activeDocument()
        source = document.activeNode() if document else None
        if not document or not source:
            self.status.setText("Select a paint layer in an open document first.")
            return
        try:
            existing = krita.find_node(document, HAIR_GUIDE_LAYER)
            if existing:
                existing.remove()
            width, height = int(document.width()), int(document.height())
            source_pixels = krita.read_pixels_rgba(source, 0, 0, width, height)
            surface = PixelSurface.blank(width, height, source=source_pixels)
            context = PixelContext(surface, document, source)
            count = load("hair-guides")().render(
                context,
                self.settings["density"],
                self.settings["length"],
                self.settings["direction"],
                self.settings["gravity"],
                self.settings["color"],
                seed="{}:{}".format(document.name(), source.name()),
            )
            guide = krita.create_guide_below(
                document, source, HAIR_GUIDE_LAYER, self.settings["opacity"]
            )
            krita.fill_layer_with_image(guide, surface.image(), document)
            self.status.setText("Applied {} Cakewalk hair guides.".format(count))
        except Exception as error:
            self.status.setText(
                "Cakewalk could not apply hair guides: {}".format(error)
            )

    def clear_hair(self):
        document = Krita.instance().activeDocument()
        layer = krita.find_node(document, HAIR_GUIDE_LAYER) if document else None
        if layer:
            layer.remove()
            document.refreshProjection()
        self.status.setText("Cleared Cakewalk hair guides.")

    def _plug_directory(self):
        return user_directory("cakewalk_modifier")

    def reload_brushes(self):
        """Rebuild the picker so added user programs become immediately usable."""
        if self.tool_panel is not None:
            self.tool_panel.disable_canvas_tool()
            self.brush_layout.removeWidget(self.tool_panel)
            self.tool_panel.deleteLater()
            self.tool_panel = None
        try:
            tools = listed_tools(self._plug_directory())
            self.tool_panel = ToolPanel(tools, self.brush_page)
            self.brush_layout.addWidget(self.tool_panel)
            self.brush_status.setText(
                "{} default brushes plus user brushes from {}. Select one, then enable its manual canvas tool.".format(
                    len(tools), self._plug_directory()
                )
            )
        except Exception as error:
            self.brush_status.setText("Could not load Cakewalk brushes: {}".format(error))

    def create_user_brush(self):
        try:
            path = write_template("my_brush", self._plug_directory())
            self.brush_status.setText("Created {}. Edit it, then reload brushes.".format(path))
        except FileExistsError:
            self.brush_status.setText("my_brush.py already exists. Edit it, then reload brushes.")
        except Exception as error:
            self.brush_status.setText("Could not create user brush template: {}".format(error))

    def closeEvent(self, event):
        if self.tool_panel is not None:
            self.tool_panel.disable_canvas_tool()
        super().closeEvent(event)


Krita.instance().addDockWidgetFactory(
    DockWidgetFactory(
        "cakewalk_modifier", DockWidgetFactoryBase.DockRight, CakewalkDocker
    )
)
