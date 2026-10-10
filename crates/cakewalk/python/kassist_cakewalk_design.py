"""Qt-backed text and shape primitives shared by design-oriented plugins."""

from PyQt5.QtCore import QRectF, Qt
from PyQt5.QtGui import QColor, QFont, QFontDatabase, QImage, QPainter, QPen, QTextDocument


def available_font_families():
    """Return host fonts, including application fonts registered at runtime."""
    return tuple(QFontDatabase().families())


def register_font_file(path):
    """Register a downloaded Google Font TTF or OTF for this Krita session."""
    font_id = QFontDatabase.addApplicationFont(str(path))
    if font_id < 0:
        raise RuntimeError("Qt could not load this font file")
    families = QFontDatabase.applicationFontFamilies(font_id)
    if not families:
        raise RuntimeError("the registered font has no usable family")
    return families[0]


def paint_shape(image, kind, bounds, fill, stroke, stroke_width=1.0, points=()):
    """Paint a rectangle, ellipse, line, or polygon onto an RGBA QImage."""
    painter = QPainter(image)
    painter.setRenderHint(QPainter.Antialiasing)
    painter.setBrush(QColor(fill) if fill else Qt.NoBrush)
    painter.setPen(QPen(QColor(stroke), float(stroke_width)) if stroke else Qt.NoPen)
    rect = QRectF(*bounds)
    if kind == "Rectangle":
        painter.drawRect(rect)
    elif kind == "Ellipse":
        painter.drawEllipse(rect)
    elif kind == "Line":
        painter.drawLine(rect.topLeft(), rect.bottomRight())
    elif kind == "Polygon":
        painter.drawPolygon(points)
    else:
        painter.end()
        raise ValueError("unknown shape kind: {}".format(kind))
    painter.end()


def rich_text_image(html, family, point_size, color, width, height):
    """Render HTML while retaining Qt's native rich-text formatting rules."""
    image = QImage(max(1, int(width)), max(1, int(height)), QImage.Format_RGBA8888)
    image.fill(Qt.transparent)
    document = QTextDocument()
    document.setDefaultFont(QFont(family, max(1, int(point_size))))
    document.setDefaultStyleSheet("body { color: %s; }" % color)
    document.setHtml(html)
    document.setTextWidth(image.width())
    painter = QPainter(image)
    document.drawContents(painter, QRectF(0, 0, image.width(), image.height()))
    painter.end()
    return image
