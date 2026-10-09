import math
from ctypes import POINTER, byref, c_bool, c_float, c_uint32

from krita import DockWidget
from PyQt5.QtCore import QPointF, Qt, pyqtSignal
from PyQt5.QtGui import QColor, QImage, QPainter, QPen
from PyQt5.QtWidgets import (
    QComboBox,
    QDoubleSpinBox,
    QFormLayout,
    QHBoxLayout,
    QInputDialog,
    QLabel,
    QListWidget,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from . import kassist_krita as krita
from .kassist_components import Panel as ComponentPanel
from .kassist_prelude import load_native_library
from .license import activate, is_licensed

GUIDE_LAYER_NAME = "SimAssist Guides"


class NativeSimAssist:
    def __init__(self):
        self.library = None
        self.error = None
        try:
            library = load_native_library("simassist")
            version = library.kassist_simassist_bridge_version
            version.restype = c_uint32
            if version() != 1:
                raise RuntimeError("unsupported SimAssist native bridge version")
            mirror = library.kassist_simassist_mirror_vertical
            mirror.argtypes = [
                c_float,
                c_float,
                c_float,
                POINTER(c_float),
                POINTER(c_float),
            ]
            mirror.restype = c_bool
            mirror_line = library.kassist_simassist_mirror_line
            mirror_line.argtypes = [
                c_float,
                c_float,
                c_float,
                c_float,
                c_float,
                c_float,
                POINTER(c_float),
                POINTER(c_float),
            ]
            mirror_line.restype = c_bool
            self.library = library
            self._mirror_vertical = mirror
            self._mirror_line = mirror_line
        except (OSError, AttributeError, RuntimeError) as error:
            self.error = str(error)

    def mirror_vertical(self, x, y, axis_x):
        if not self.library:
            raise RuntimeError(self.error or "SimAssist native bridge is unavailable")
        output_x, output_y = c_float(), c_float()
        if not self._mirror_vertical(
            c_float(x), c_float(y), c_float(axis_x), byref(output_x), byref(output_y)
        ):
            raise RuntimeError("SimAssist native bridge rejected the point")
        return output_x.value, output_y.value

    def mirror_axis(self, x, y, axis):
        if axis["kind"] == "Vertical":
            return self.mirror_vertical(x, y, axis["x"])
        if axis["kind"] == "Horizontal":
            start_x, start_y = 0.0, axis["y"]
            end_x, end_y = 1.0, axis["y"]
        else:
            start_x, start_y = axis["x"], axis["y"]
            end_x = start_x + math.cos(axis["angle"])
            end_y = start_y + math.sin(axis["angle"])
        if not self.library:
            raise RuntimeError(self.error or "SimAssist native bridge is unavailable")
        output_x, output_y = c_float(), c_float()
        if not self._mirror_line(
            c_float(x),
            c_float(y),
            c_float(start_x),
            c_float(start_y),
            c_float(end_x),
            c_float(end_y),
            byref(output_x),
            byref(output_y),
        ):
            raise RuntimeError("SimAssist native bridge rejected the axis")
        return output_x.value, output_y.value


class AnglePicker(QWidget):
    angleChanged = pyqtSignal(float)

    def __init__(self, angle=0.0, parent=None):
        super().__init__(parent)
        self._angle = angle
        self.setMinimumSize(120, 120)
        self.setCursor(Qt.CrossCursor)

    def angle(self):
        return self._angle

    def _set_from_position(self, position):
        center = QPointF(self.width() / 2.0, self.height() / 2.0)
        self._angle = math.atan2(position.y() - center.y(), position.x() - center.x())
        self.update()
        self.angleChanged.emit(self._angle)

    def mousePressEvent(self, event):
        self._set_from_position(event.pos())

    def mouseMoveEvent(self, event):
        if event.buttons() & Qt.LeftButton:
            self._set_from_position(event.pos())

    def paintEvent(self, event):
        del event
        painter = QPainter(self)
        painter.setRenderHint(QPainter.Antialiasing)
        center = QPointF(self.width() / 2.0, self.height() / 2.0)
        radius = max(12.0, min(self.width(), self.height()) / 2.0 - 10.0)
        painter.setPen(QPen(QColor("#777777"), 1))
        painter.drawEllipse(center, radius, radius)
        endpoint = QPointF(
            center.x() + math.cos(self._angle) * radius,
            center.y() + math.sin(self._angle) * radius,
        )
        painter.setPen(QPen(QColor("#4ea1ff"), 3))
        painter.drawLine(center, endpoint)
        painter.end()


class SimAssistDocker(DockWidget):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("SimAssist")
        self.axes = []
        self.native = NativeSimAssist()

        content = QWidget(self)
        layout = QVBoxLayout(content)
        layout.addWidget(
            QLabel("Reference guides are placed on a separate paint layer.")
        )
        activate_button = QPushButton("Support with Gumroad")
        activate_button.clicked.connect(self.activate_license)
        layout.addWidget(activate_button)

        form = QFormLayout()
        self.axis_kind = QComboBox()
        self.axis_kind.addItems(["Vertical", "Horizontal", "Free angle"])
        self.axis_x = self._coordinate_spin()
        self.axis_y = self._coordinate_spin()
        self.angle_degrees = self._coordinate_spin(-360.0, 360.0)
        self.angle_degrees.setSuffix("°")
        form.addRow("Axis", self.axis_kind)
        form.addRow("Anchor X", self.axis_x)
        form.addRow("Anchor Y", self.axis_y)
        form.addRow("Angle", self.angle_degrees)
        layout.addLayout(form)

        self.picker = AnglePicker()
        self.picker.angleChanged.connect(self._set_angle_from_picker)
        layout.addWidget(self.picker)

        add_axis = QPushButton("Add axis")
        add_axis.clicked.connect(self.add_axis)
        remove_axis = QPushButton("Remove selected axis")
        remove_axis.clicked.connect(self.remove_axis)
        axis_buttons = QHBoxLayout()
        axis_buttons.addWidget(add_axis)
        axis_buttons.addWidget(remove_axis)
        layout.addLayout(axis_buttons)
        self.axis_list = QListWidget()
        layout.addWidget(self.axis_list)

        self.radial_segments = 0
        ComponentPanel("Radial symmetry", self._dispatch_action).number(
            "Radial copies (0 disables)", 0.0, 0.0, 24.0, 1.0, 0, "set_radial_segments"
        ).mount(layout)

        sample = QFormLayout()
        self.sample_x = self._coordinate_spin()
        self.sample_y = self._coordinate_spin()
        sample.addRow("Sample point X", self.sample_x)
        sample.addRow("Sample point Y", self.sample_y)
        layout.addLayout(sample)
        mirror = QPushButton("Mirror sample across selected axis")
        mirror.clicked.connect(self.mirror_sample)
        layout.addWidget(mirror)

        render = QPushButton("Render / update guides")
        render.clicked.connect(self.render_guides)
        clear = QPushButton("Clear SimAssist guides")
        clear.clicked.connect(self.clear_guides)
        layout.addWidget(render)
        layout.addWidget(clear)
        self.status = QLabel("Add axes, then render a reference layer.")
        layout.addWidget(self.status)
        self.setWidget(content)

    def _dispatch_action(self, action, value=None):
        if action == "set_radial_segments":
            self.radial_segments = round(value)

    @staticmethod
    def _coordinate_spin(minimum=-100000.0, maximum=100000.0):
        control = QDoubleSpinBox()
        control.setRange(minimum, maximum)
        control.setDecimals(1)
        control.setSingleStep(10.0)
        return control

    def _set_angle_from_picker(self, radians):
        self.angle_degrees.setValue(math.degrees(radians))

    def canvasChanged(self, canvas):
        if not canvas:
            return
        view = canvas.view() if hasattr(canvas, "view") else None
        document = view.document() if view is not None else krita.active_document()
        if document is None:
            return
        if not self.axes:
            self.axis_x.setValue(document.width() / 2.0)
            self.axis_y.setValue(document.height() / 2.0)

    def add_axis(self):
        if not is_licensed():
            self.status.setText("License required (or compile with --dev).")
            return
        kind = self.axis_kind.currentText()
        axis = {
            "kind": kind,
            "x": self.axis_x.value(),
            "y": self.axis_y.value(),
            "angle": math.radians(self.angle_degrees.value()),
        }
        self.axes.append(axis)
        self.axis_list.addItem(self._axis_label(axis))

    def activate_license(self):
        key, accepted = QInputDialog.getText(self, "Support SimAssist", "Gumroad receipt key")
        if not accepted:
            return
        _ok, message = activate(key)
        self.status.setText(message)

    def remove_axis(self):
        row = self.axis_list.currentRow()
        if row < 0:
            return
        self.axes.pop(row)
        self.axis_list.takeItem(row)

    def mirror_sample(self):
        row = self.axis_list.currentRow()
        if row < 0 or row >= len(self.axes):
            self.status.setText("Select an axis before mirroring a sample point.")
            return
        try:
            result = self.native.mirror_axis(
                self.sample_x.value(), self.sample_y.value(), self.axes[row]
            )
            self.status.setText(
                "Native mirror result: ({:.1f}, {:.1f})".format(*result)
            )
        except RuntimeError as error:
            self.status.setText(f"Native bridge unavailable: {error}")

    @staticmethod
    def _axis_label(axis):
        if axis["kind"] == "Free angle":
            return "Free angle at ({:.0f}, {:.0f}), {:.0f}°".format(
                axis["x"], axis["y"], math.degrees(axis["angle"])
            )
        value = axis["x"] if axis["kind"] == "Vertical" else axis["y"]
        return "{} at {:.0f}".format(axis["kind"], value)

    def render_guides(self):
        if not is_licensed():
            self.status.setText("License required (or compile with --dev).")
            return
        document = krita.active_document()
        if not document:
            self.status.setText("Open a document before rendering guides.")
            return
        if not self.axes:
            self.status.setText("Add at least one axis first.")
            return
        image = QImage(document.width(), document.height(), QImage.Format_RGBA8888)
        image.fill(Qt.transparent)
        painter = QPainter(image)
        painter.setRenderHint(QPainter.Antialiasing)
        painter.setPen(QPen(QColor(78, 161, 255, 210), 2))
        for axis in self.axes:
            self._draw_axis(painter, axis, document.width(), document.height())
        self._draw_radial(painter, document.width(), document.height())
        self._draw_perspective(painter, document.width(), document.height())
        painter.end()
        layer = self._guide_layer(document)
        if not layer:
            layer = krita.add_paint_layer(document, GUIDE_LAYER_NAME)
        try:
            krita.fill_layer_with_image(layer, image, document)
        except RuntimeError as error:
            self.status.setText(str(error))
            return
        layer.setOpacity(170)
        krita.set_active_node(layer, document)
        document.refreshProjection()
        self.status.setText(
            f"Updated {len(self.axes)} axes and {self.radial_segments} radial copies."
        )

    def clear_guides(self):
        document = krita.active_document()
        if not document:
            return
        layer = self._guide_layer(document)
        if layer:
            layer.remove()
            document.refreshProjection()
        self.status.setText("Cleared SimAssist guides.")

    @staticmethod
    def _draw_axis(painter, axis, width, height):
        if axis["kind"] == "Vertical":
            painter.drawLine(QPointF(axis["x"], 0), QPointF(axis["x"], height))
        elif axis["kind"] == "Horizontal":
            painter.drawLine(QPointF(0, axis["y"]), QPointF(width, axis["y"]))
        else:
            distance = math.hypot(width, height) * 2.0
            dx, dy = math.cos(axis["angle"]), math.sin(axis["angle"])
            painter.drawLine(
                QPointF(axis["x"] - dx * distance, axis["y"] - dy * distance),
                QPointF(axis["x"] + dx * distance, axis["y"] + dy * distance),
            )

    def _draw_radial(self, painter, width, height):
        segments = self.radial_segments
        if segments < 2:
            return
        center_x, center_y = self.axis_x.value(), self.axis_y.value()
        radius = math.hypot(width, height)
        start = math.radians(self.angle_degrees.value())
        painter.setPen(QPen(QColor(255, 190, 78, 180), 1))
        for index in range(segments):
            angle = start + (math.tau * index / segments)
            painter.drawLine(
                QPointF(center_x, center_y),
                QPointF(
                    center_x + math.cos(angle) * radius,
                    center_y + math.sin(angle) * radius,
                ),
            )

    def _draw_perspective(self, painter, width, height):
        lines = [axis for axis in self.axes if axis["kind"] == "Free angle"]
        if len(lines) < 2:
            return
        point = self._intersection(lines[0], lines[1])
        if point is None:
            return
        painter.setPen(QPen(QColor(120, 230, 150, 180), 1))
        x, y = point
        for edge in range(0, width + 1, max(1, width // 8)):
            painter.drawLine(QPointF(x, y), QPointF(edge, 0))
            painter.drawLine(QPointF(x, y), QPointF(edge, height))

    @staticmethod
    def _intersection(first, second):
        first_direction = (math.cos(first["angle"]), math.sin(first["angle"]))
        second_direction = (math.cos(second["angle"]), math.sin(second["angle"]))
        denominator = (
            first_direction[0] * second_direction[1]
            - first_direction[1] * second_direction[0]
        )
        if abs(denominator) < 1e-6:
            return None
        dx, dy = second["x"] - first["x"], second["y"] - first["y"]
        distance = (dx * second_direction[1] - dy * second_direction[0]) / denominator
        return (
            first["x"] + distance * first_direction[0],
            first["y"] + distance * first_direction[1],
        )

    @staticmethod
    def _guide_layer(document):
        return krita.find_node(document, GUIDE_LAYER_NAME)


krita.register_dock_widget("simassist", SimAssistDocker)
