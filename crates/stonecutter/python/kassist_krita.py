import contextlib
import os
import sys

from PyQt5.QtCore import QCoreApplication, QEvent, QObject, QPointF, QSettings, QStandardPaths, QUrl
from PyQt5.QtGui import (
    QBrush,
    QColor,
    QDesktopServices,
    QImage,
    QPainter,
    QRadialGradient,
)

try:
    from krita import Extension
except ImportError:
    Extension = object

RGBA = "rgba"
ELEMENT_BYTES = {"u8": 1, "u16": 2, "float16": 2, "float32": 4, 8: 1, 16: 2, 32: 4}


def _krita():
    from krita import Krita

    return Krita.instance()


def instance():
    """Return the running Krita application."""
    return _krita()


def version():
    """Return Krita's version as a tuple of integers, or () when unknown."""
    reported = _krita().version()
    try:
        return tuple(int(part) for part in reported)
    except (TypeError, ValueError):
        return ()


def documents():
    """Return every open document."""
    return list(_krita().documents())


def active_document():
    """Return the active document, or None when Krita has none."""
    return _krita().activeDocument()


def require_document():
    """Return the active document or raise when Krita has none."""
    document = active_document()
    if document is None:
        raise RuntimeError("no Krita document is open")
    return document


def active_node(document=None):
    """Return the active layer of a document, or None."""
    document = document if document is not None else require_document()
    return document.activeNode()


def require_node(document=None):
    """Return the active layer or raise when the document has none."""
    node = active_node(document)
    if node is None:
        raise RuntimeError("the active layer is missing")
    return node


def set_active_node(node, document=None):
    """Make a layer active, raising when either document or layer is missing."""
    document = document if document is not None else require_document()
    if node is None:
        raise RuntimeError("cannot activate a missing layer")
    document.setActiveNode(node)
    return node


def children(node):
    """Return a layer's direct children across PyKrita spellings."""
    for name in ("childNodes", "children"):
        method = getattr(node, name, None)
        if method is not None:
            return list(method())
    return []


def walk(node):
    """Yield a layer and all of its descendants, parents first."""
    stack = [node]
    while stack:
        current = stack.pop()
        yield current
        stack.extend(reversed(children(current)))


def find_node(document, name):
    """Return the first layer with a name, or None."""
    for node in walk(document.rootNode()):
        if node.name() == name:
            return node
    return None


def find_nodes(document, kind):
    """Return every layer whose type matches, such as 'paintLayer'."""
    return [node for node in walk(document.rootNode()) if node.type() == kind]


def add_paint_layer(document, name):
    """Add an empty paint layer to the document root and return it."""
    layer = _create_paint_layer(document, name)
    if not document.rootNode().addChildNode(layer, None):
        raise RuntimeError("Krita could not add the new paint layer")
    return layer


def add_group_layer(document, name):
    """Add an empty group layer to the document root and return it."""
    layer = document.createGroupLayer(name)
    document.rootNode().addChildNode(layer, None)
    return layer


def add_vector_layer(document, name):
    """Add an empty vector layer to the document root and return it."""
    layer = document.createVectorLayer(name)
    document.rootNode().addChildNode(layer, None)
    return layer


@contextlib.contextmanager
def active_node_guard(document=None):
    """Run a block and restore the layer that was active before it."""
    document = document if document is not None else require_document()
    previous = document.activeNode()
    try:
        yield document
    finally:
        if previous is not None:
            document.setActiveNode(previous)


def trigger_action(name):
    """Run a Krita action; return False when it is missing or disabled."""
    action = _krita().action(name)
    if action is None or not action.isEnabled():
        return False
    action.trigger()
    return True


def require_action(name):
    """Run a Krita action or raise a RuntimeError naming it."""
    if not trigger_action(name):
        raise RuntimeError("Krita's " + name + " action is unavailable")


def brush_presets():
    """Return the names of every installed brush preset."""
    return [resource.name() for resource in _krita().resources("brushes")]


def create_guide_below(document, selected, name, opacity=96):
    """Create a semi-transparent paint layer immediately below `selected`."""
    if selected is None:
        raise RuntimeError("select a paint layer before creating a guide")
    parent = selected.parentNode() or document.rootNode()
    layer = _create_paint_layer(document, name)
    if not parent.addChildNode(layer, selected):
        raise RuntimeError("Krita could not insert the guide layer")
    layer.setOpacity(max(0, min(255, int(opacity))))
    return layer


def _create_paint_layer(document, name):
    """Create a paint layer across PyKrita versions.

    Public PyKrita exposes Document.createNode(name, "paintlayer"); some
    builds do not expose the historical createPaintLayer convenience method.
    """
    create = getattr(document, "createNode", None)
    layer = create(name, "paintlayer") if create else None
    if layer is None:
        legacy = getattr(document, "createPaintLayer", None)
        layer = legacy(name) if legacy else None
    if layer is None:
        raise RuntimeError("Krita could not create a paint layer for this document")
    return layer


def paint_pressure_line(node, start, end, pressure_start=1.0, pressure_end=1.0):
    """Paint through Krita's native pressure-aware Node.paintLine API."""
    start_pressure = max(0.0, min(1.0, float(pressure_start)))
    end_pressure = max(0.0, min(1.0, float(pressure_end)))
    return node.paintLine(
        QPointF(float(start[0]), float(start[1])),
        QPointF(float(end[0]), float(end[1])),
        start_pressure,
        end_pressure,
    )


class PressureCanvas(QObject):
    """Qt tablet-event capture for manual canvas tools and guide previews."""

    def __init__(self, widget, callback=None, parent=None):
        super().__init__(parent or widget)
        self.widget = widget
        self.callback = callback
        self.samples = []
        self.latest_pressure = 1.0
        widget.installEventFilter(self)

    def detach(self):
        self.widget.removeEventFilter(self)

    def eventFilter(self, watched, event):
        if watched is self.widget and event.type() in (
            QEvent.TabletPress,
            QEvent.TabletMove,
            QEvent.TabletRelease,
        ):
            position = event.position() if hasattr(event, "position") else event.posF()
            sample = {
                "x": float(position.x()),
                "y": float(position.y()),
                "pressure": max(0.0, min(1.0, float(event.pressure()))),
                "kind": int(event.type()),
            }
            self.latest_pressure = sample["pressure"]
            self.samples.append(sample)
            del self.samples[:-256]
            if self.callback:
                self.callback(sample)
        return False


def active_canvas_widget():
    """Best-effort access to Krita's current Qt canvas widget."""
    window = _krita().activeWindow()
    view = window.activeView() if window else None
    canvas = getattr(view, "canvas", None) if view else None
    canvas = canvas() if callable(canvas) else canvas
    for candidate in (view, canvas):
        if candidate is None:
            continue
        widget = getattr(candidate, "widget", lambda: candidate)()
        if hasattr(widget, "installEventFilter"):
            return widget
    return None


def capture_pressure(callback=None, widget=None):
    """Attach a `PressureCanvas` to a supplied or active Krita canvas widget."""
    widget = widget or active_canvas_widget()
    if widget is None:
        raise RuntimeError("Krita did not expose an active canvas widget")
    capture = PressureCanvas(widget, callback)
    widget._kassist_pressure_capture = capture
    return capture


class BrushRecipe:
    """A portable description for modifying the active Krita brush."""

    def __init__(self, preset=None, size=None, opacity=None, color=None):
        self.preset = preset
        self.size = size
        self.opacity = opacity
        self.color = color

    def apply(self, view=None):
        view = view or _krita().activeWindow().activeView()
        if view is None:
            raise RuntimeError("Krita has no active view")
        if self.size is not None and hasattr(view, "setBrushSize"):
            view.setBrushSize(float(self.size))
        if self.opacity is not None:
            setter = getattr(view, "setPaintingOpacity", None)
            if setter:
                setter(float(self.opacity))
        if self.color is not None and hasattr(view, "setForeGroundColor"):
            view.setForeGroundColor(QColor(self.color))
        if self.preset is not None:
            resource = next(
                (
                    item
                    for item in _krita().resources("brushes")
                    if item.name() == self.preset
                ),
                None,
            )
            setter = getattr(view, "setCurrentBrushPreset", None)
            if resource is None or setter is None:
                raise RuntimeError(
                    "the requested brush preset is unavailable in this Krita build"
                )
            setter(resource)
        return self

    def stamp(self, diameter=128, softness=0.7):
        """Generate a brush stamp image usable by a manual canvas tool."""
        diameter = max(2, int(diameter))
        softness = max(0.0, min(1.0, float(softness)))
        image = QImage(diameter, diameter, QImage.Format_RGBA8888)
        image.fill(0)
        painter = QPainter(image)
        color = QColor(self.color or "#000000")
        gradient = QRadialGradient(diameter / 2.0, diameter / 2.0, diameter / 2.0)
        solid = QColor(color)
        edge = QColor(color)
        edge.setAlpha(0)
        solid.setAlpha(round(255 * (self.opacity if self.opacity is not None else 1.0)))
        gradient.setColorAt(0.0, solid)
        gradient.setColorAt(max(0.0, min(0.99, softness)), solid)
        gradient.setColorAt(1.0, edge)
        painter.fillRect(0, 0, diameter, diameter, QBrush(gradient))
        painter.end()
        return image


def notify(message, title="kassist"):
    """Show a message to the user, preferring Krita's notification API."""
    application = _krita()
    for name in ("notify", "notification"):
        method = getattr(application, name, None)
        if method is None:
            continue
        try:
            method(title, message)
            return True
        except TypeError:
            try:
                method(message)
                return True
            except Exception:
                continue
        except Exception:
            continue
    application.messageBox(title, message)
    return True


def bytes_per_pixel(document):
    """Return how many bytes Krita stores per pixel for a document."""
    depth = document.colorDepth()
    if depth not in ELEMENT_BYTES:
        raise RuntimeError(f"unsupported color depth: {depth!r}")
    return 4 * ELEMENT_BYTES[depth]


def read_pixels(node, x=0, y=0, width=None, height=None):
    """Return a layer's pixels as bytes in Krita's native channel order."""
    width = node.width() if width is None else width
    height = node.height() if height is None else height
    return node.pixelData(x, y, width, height)


def write_pixels(node, x, y, width, height, pixels, document=None):
    """Write bytes to a layer, converting RGBA to Krita's native order."""
    depth = bytes_per_pixel(document if document is not None else owning_document(node))
    _check_length(pixels, width, height, depth)
    written = node.setPixelData(_to_native(pixels), x, y, width, height)
    if written is False:
        raise RuntimeError("Krita could not write pixels to this node")


def read_pixels_rgba(node, x=0, y=0, width=None, height=None):
    """Return a layer's pixels as RGBA bytes."""
    width = node.width() if width is None else width
    height = node.height() if height is None else height
    return _to_rgba(read_pixels(node, x, y, width, height))


def write_pixels_rgba(node, x, y, width, height, pixels, document=None):
    """Write RGBA bytes to a layer."""
    depth = bytes_per_pixel(document if document is not None else owning_document(node))
    _check_length(pixels, width, height, depth)
    if depth != 4:
        raise RuntimeError("RGBA pixel access needs an 8-bit document")
    written = node.setPixelData(_to_native(pixels), x, y, width, height)
    if written is False:
        raise RuntimeError("Krita could not write pixels to this node")


def owning_document(node):
    """Return the document that owns a layer."""
    for name in ("document", "parentDocument"):
        method = getattr(node, name, None)
        if method is not None:
            return method()
    raise RuntimeError("Krita does not expose the document of this layer")


def _check_length(pixels, width, height, depth):
    expected = width * height * depth
    if len(pixels) != expected:
        raise ValueError(
            f"expected {expected} bytes for {width}x{height} pixels, got {len(pixels)}"
        )


def image_rgba(image):
    """Return a QImage's pixels as RGBA bytes."""
    converted = image.convertToFormat(QImage.Format_RGBA8888)
    return bytes(converted.bits().asstring(converted.sizeInBytes()))


def fill_layer_with_image(layer, image, document=None):
    """Replace a layer's pixels with a QImage."""
    image = image.convertToFormat(QImage.Format_RGBA8888)
    write_pixels_rgba(
        layer, 0, 0, image.width(), image.height(), image_rgba(image), document
    )
    layer.refreshProjection(0, 0, image.width(), image.height())
    if document is not None:
        document.refreshProjection()
    return layer


def fill_layer_with_color(layer, color, document=None):
    """Replace a layer's pixels with a single color."""
    width, height = layer.width(), layer.height()
    image = QImage(width, height, QImage.Format_RGBA8888)
    image.fill(QColor(color))
    return fill_layer_with_image(layer, image, document)


def place_image_as_new_layer(document, image, name="Image"):
    """Add a paint layer holding a QImage."""
    layer = add_paint_layer(document, name)
    return fill_layer_with_image(layer, image, document)


def place_image_as_reference(document, image, name="Reference image"):
    """Create a native Krita reference image from a QImage.

    Krita exposes this only as an action.  The source is therefore placed on a
    standard temporary paint layer, copied after its projection is current,
    and converted through the native *Paste as Reference Image* action.
    """
    if image.isNull():
        raise ValueError("cannot create a reference image from an empty image")

    def flush():
        application = QCoreApplication.instance()
        if application is not None:
            application.processEvents()
        wait = getattr(document, "waitForDone", None)
        if callable(wait):
            wait()

    def run_action(*names):
        flush()
        application = _krita()
        for action_name in names:
            action = application.action(action_name)
            if action is None:
                continue
            if not action.isEnabled():
                flush()
            if action.isEnabled():
                action.trigger()
                flush()
                return action_name
        raise RuntimeError("Krita's " + " / ".join(names) + " action is unavailable")

    with active_node_guard(document):
        layer = add_paint_layer(document, name)
        try:
            fill_layer_with_image(layer, image, document)
            set_active_node(layer, document)
            flush()
            run_action("select_all")
            run_action("copy")
            run_action("paste_as_reference_image", "paste_as_reference")
        finally:
            layer.remove()
            document.refreshProjection()
            flush()
    return document


def color_from_hex(value, alpha=255):
    """Return a QColor from '#rrggbb' or '#rrggbbaa'."""
    color = QColor(value)
    if not color.isValid():
        raise ValueError(f"not a color: {value!r}")
    if len(str(value).lstrip("#")) == 6:
        color.setAlpha(alpha)
    return color


def hex_color(color):
    """Return '#rrggbbaa' for a QColor."""
    return f"#{color.red():02x}{color.green():02x}{color.blue():02x}{color.alpha():02x}"


def app_data_dir(*parts):
    """Return, and create, a directory inside Krita's application data."""
    root = QStandardPaths.writableLocation(QStandardPaths.AppDataLocation)
    path = os.path.join(root, *parts) if parts else root
    os.makedirs(path, exist_ok=True)
    return path


class Settings:
    """Typed plugin settings stored in Krita's QSettings database."""

    def __init__(self, plugin_id):
        self._settings = QSettings("Hellings", "kassist")
        self._prefix = "plugins/" + plugin_id + "/"

    def get(self, key, default=None, kind=str):
        """Read a setting, falling back to `default`."""
        if default is not None and not isinstance(default, kind):
            raise TypeError("default must be a " + kind.__name__)
        return self._settings.value(self._prefix + key, default, type=kind)

    def set(self, key, value):
        """Write a setting of a supported type."""
        if not isinstance(value, (bool, int, float, str)):
            raise TypeError("unsupported setting type")
        self._settings.setValue(self._prefix + key, value)
        return value

    def remove(self, key):
        """Forget a setting."""
        self._settings.remove(self._prefix + key)

    def stored(self):
        """Return every setting this plugin owns."""
        return self._settings.allKeys()


class KassistExtension(Extension):
    """Base class for a Krita extension with declarative menus.

    Subclasses list menu paths and their actions in `MENUS`; each callback is
    either a method name on the subclass or a plain callable.
    """

    MENUS = {}

    def setup(self):
        pass

    def createActions(self, window):
        for path in sorted(self.MENUS):
            menu = window.createMenu(path)
            if menu is None:
                continue
            for entry in self.MENUS[path]:
                name, text, callback = entry
                action = window.createAction(name, text)
                handler = (
                    getattr(self, callback) if isinstance(callback, str) else callback
                )
                action.triggered.connect(
                    lambda _checked=False, handler=handler: handler()
                )
                menu.addAction(action)

    def activate(self, window):
        pass

    def deactivate(self, window):
        pass


def install(extension_class):
    """Register an Extension subclass with the running Krita instance."""
    _krita().addExtension(extension_class(_krita()))
    return extension_class


def register_dock_widget(factory_name, widget_class, location=None):
    """Register a DockWidget subclass as a Krita dock widget."""
    from krita import DockWidgetFactory, DockWidgetFactoryBase

    if location is None:
        location = DockWidgetFactoryBase.DockRight
    _krita().addDockWidgetFactory(
        DockWidgetFactory(factory_name, location, widget_class)
    )


def open_url(url):
    """Open a URL with the desktop's default handler."""
    return QDesktopServices.openUrl(QUrl(url))


def _swap_channels(pixels):
    swapped = bytearray(pixels)
    if len(swapped) % 4:
        raise RuntimeError("pixel data is not a multiple of four bytes")
    swapped[0::4], swapped[2::4] = swapped[2::4], swapped[0::4]
    return bytes(swapped)


def _to_native(pixels):
    return _swap_channels(pixels) if sys.byteorder == "little" else bytes(pixels)


def _to_rgba(pixels):
    return _swap_channels(pixels) if sys.byteorder == "little" else bytes(pixels)
