"""GDesign rich text, intentionally focused on a single native-style workflow."""

from krita import DockWidget
from PyQt5.QtGui import QFont, QTextCharFormat
from PyQt5.QtWidgets import QComboBox, QFileDialog, QHBoxLayout, QLabel, QPushButton, QSpinBox, QTextEdit, QVBoxLayout, QWidget

from . import kassist_krita as krita
from .kassist_cakewalk_design import available_font_families, register_font_file, rich_text_image

TEXT_LAYER = "GDesign Text"


class GDesignDocker(DockWidget):
    """A compact rich-text editor which writes one dedicated design layer."""
    def __init__(self):
        super().__init__()
        self.setWindowTitle("GDesign Text")
        content = QWidget(self)
        layout = QVBoxLayout(content)
        layout.setContentsMargins(8, 8, 8, 8)
        self.status = QLabel("Write text, format a selection, then update the text layer.", content)
        self.status.setWordWrap(True)
        layout.addWidget(self.status)
        bar = QHBoxLayout()
        self.family = QComboBox(content)
        self.family.setEditable(True)
        self.family.addItems(available_font_families())
        self.size = QSpinBox(content)
        self.size.setRange(6, 512)
        self.size.setValue(42)
        bold = QPushButton("B", content)
        bold.clicked.connect(self.toggle_bold)
        font = QPushButton("Font…", content)
        font.clicked.connect(self.load_font)
        bar.addWidget(self.family, 1)
        bar.addWidget(self.size)
        bar.addWidget(bold)
        bar.addWidget(font)
        layout.addLayout(bar)
        self.editor = QTextEdit(content)
        self.editor.setPlaceholderText("Type here. Select part of the text to update only that selection.")
        self.editor.setMinimumHeight(180)
        layout.addWidget(self.editor)
        update = QPushButton("Update text layer", content)
        update.clicked.connect(self.commit)
        layout.addWidget(update)
        self.setWidget(content)

    def canvasChanged(self, canvas):
        self.canvas = canvas
        if canvas is None:
            self.status.setText("Open a document to place rich text.")

    def toggle_bold(self):
        cursor = self.editor.textCursor()
        format_ = QTextCharFormat()
        format_.setFontWeight(QFont.Normal if cursor.charFormat().fontWeight() > QFont.Normal else QFont.Bold)
        cursor.mergeCharFormat(format_)

    def load_font(self):
        path, _ = QFileDialog.getOpenFileName(self, "Load a Google Font file", "", "Fonts (*.ttf *.otf)")
        if not path:
            return
        try:
            family = register_font_file(path)
            self.family.addItem(family)
            self.family.setCurrentText(family)
            self.status.setText("Loaded {} for this Krita session.".format(family))
        except Exception as error:
            self.status.setText("Could not load font: {}".format(error))

    def commit(self):
        document = krita.active_document()
        if not document:
            self.status.setText("Open a document before updating text.")
            return
        image = rich_text_image(self.editor.toHtml(), self.family.currentText(), self.size.value(), "#ffffffff", document.width(), document.height())
        layer = krita.find_node(document, TEXT_LAYER) or krita.add_paint_layer(document, TEXT_LAYER)
        krita.fill_layer_with_image(layer, image, document)
        self.status.setText("Text layer updated.")


krita.register_dock_widget("gdesign", GDesignDocker)
