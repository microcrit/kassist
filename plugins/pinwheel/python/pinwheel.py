import json
import os
import uuid

from krita import DockWidget
from PyQt5.QtCore import QSize, Qt
from PyQt5.QtGui import QIcon, QImage, QPixmap
from PyQt5.QtWidgets import (
    QAbstractItemView,
    QFileDialog,
    QInputDialog,
    QLabel,
    QLineEdit,
    QListView,
    QListWidget,
    QListWidgetItem,
    QMessageBox,
    QSplitter,
    QVBoxLayout,
    QWidget,
)

from . import kassist_krita as krita
from .kassist_components import Panel as ComponentPanel
from .license import activate, is_licensed


class ReferenceStore:
    def __init__(self):
        self.path = os.path.join(krita.app_data_dir(), "pinwheel-references.json")
        self.items = self._load()

    def _load(self):
        try:
            with open(self.path, "r", encoding="utf-8") as handle:
                stored_items = json.load(handle)
        except (OSError, ValueError):
            return []
        if not isinstance(stored_items, list):
            return []
        items, ids = [], set()
        for item in stored_items:
            if not isinstance(item, dict):
                continue
            reference_id = item.get("id")
            path = item.get("path")
            tags = item.get("tags", [])
            if (
                not isinstance(reference_id, str)
                or not reference_id
                or reference_id in ids
                or not isinstance(path, str)
                or not path
                or not isinstance(tags, list)
            ):
                continue
            ids.add(reference_id)
            items.append(
                {
                    "id": reference_id,
                    "path": os.path.abspath(path),
                    "tags": [
                        tag.strip()
                        for tag in tags
                        if isinstance(tag, str) and tag.strip()
                    ],
                }
            )
        return items

    def save(self):
        temporary = self.path + ".tmp"
        with open(temporary, "w", encoding="utf-8") as handle:
            json.dump(self.items, handle, indent=2)
        os.replace(temporary, self.path)

    def add(self, path, tags):
        path = os.path.abspath(path)
        previous = self.find_by_path(path)
        if previous:
            previous["tags"] = tags
        else:
            self.items.append({"id": str(uuid.uuid4()), "path": path, "tags": tags})
        self.save()

    def find(self, reference_id):
        return next((item for item in self.items if item["id"] == reference_id), None)

    def find_by_path(self, path):
        return next((item for item in self.items if item["path"] == path), None)

    def remove(self, reference_id):
        self.items = [item for item in self.items if item["id"] != reference_id]
        self.save()


class PinwheelDocker(DockWidget):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Pinwheel references")
        self.store = ReferenceStore()
        content = QWidget(self)
        layout = QVBoxLayout(content)
        self.filter = QLineEdit(placeholderText="Filter by filename or tag")
        self.filter.textChanged.connect(self.refresh)
        self.list = QListWidget()
        self.list.setViewMode(QListView.IconMode)
        self.list.setResizeMode(QListView.Adjust)
        self.list.setMovement(QListView.Static)
        self.list.setSelectionMode(QAbstractItemView.SingleSelection)
        self.list.setIconSize(QSize(128, 128))
        self.list.setGridSize(QSize(156, 184))
        self.list.itemDoubleClicked.connect(self.pop_out)
        self.list.currentItemChanged.connect(self.preview_reference)
        self.preview = QLabel("Select a reference to review it.")
        self.preview.setAlignment(Qt.AlignCenter)
        self.preview.setMinimumWidth(180)
        self.preview.setWordWrap(True)
        self.details = QLabel("")
        self.details.setWordWrap(True)
        preview_panel = QWidget()
        preview_layout = QVBoxLayout(preview_panel)
        preview_layout.addWidget(self.preview)
        preview_layout.addWidget(self.details)
        split = QSplitter(Qt.Horizontal)
        split.addWidget(self.list)
        split.addWidget(preview_panel)
        split.setSizes([350, 220])
        layout.addWidget(self.filter)
        layout.addWidget(split)
        ComponentPanel("Pinwheel", self._dispatch_action).row(
            lambda ui: (
                ui.button("Add references", "add")
                .button("Edit tags", "edit_tags")
                .button("Remove", "remove")
                .button("Pop out on canvas", "pop_out")
            )
        ).button("Support with Gumroad", "activate").mount(layout)
        self.setWidget(content)
        self.refresh()

    def canvasChanged(self, canvas):
        self.canvas = canvas

    def _dispatch_action(self, action, value=None):
        del value
        if action == "add":
            self.add_references()
        elif action == "edit_tags":
            self.edit_tags()
        elif action == "remove":
            self.remove_selected()
        elif action == "pop_out":
            self.pop_out()
        elif action == "activate":
            self.activate_license()

    def refresh(self):
        needle = self.filter.text().strip().lower()
        previous = self._selected_reference()
        previous_id = previous["id"] if previous else None
        self.list.clear()
        for reference in self.store.items:
            label = "{}  [{}]".format(
                os.path.basename(reference["path"]), ", ".join(reference["tags"])
            )
            if needle and needle not in label.lower():
                continue
            item = QListWidgetItem(self._thumbnail(reference), label)
            item.setData(Qt.UserRole, reference["id"])
            item.setToolTip(label)
            self.list.addItem(item)
            if reference["id"] == previous_id:
                self.list.setCurrentItem(item)
        if not self.list.currentItem():
            self.preview.setPixmap(QPixmap())
            self.preview.setText("Select a reference to review it.")
            self.details.setText("")

    @staticmethod
    def _thumbnail(reference):
        pixmap = QPixmap(reference["path"])
        if pixmap.isNull():
            return QIcon()
        return QIcon(
            pixmap.scaled(128, 128, Qt.KeepAspectRatio, Qt.SmoothTransformation)
        )

    def _selected_reference(self):
        selected = self.list.currentItem()
        return self.store.find(selected.data(Qt.UserRole)) if selected else None

    def preview_reference(self, current, previous):
        del previous
        reference = self.store.find(current.data(Qt.UserRole)) if current else None
        if not reference:
            return
        pixmap = QPixmap(reference["path"])
        if pixmap.isNull():
            self.preview.setPixmap(QPixmap())
            self.preview.setText("Missing image file")
        else:
            self.preview.setText("")
            self.preview.setPixmap(
                pixmap.scaled(320, 320, Qt.KeepAspectRatio, Qt.SmoothTransformation)
            )
        self.details.setText(
            "{}\nTags: {}".format(
                reference["path"], ", ".join(reference["tags"]) or "none"
            )
        )

    def add_references(self):
        if not is_licensed():
            self.setWindowTitle("Pinwheel references")
            return
        paths, _ = QFileDialog.getOpenFileNames(
            self,
            "Choose reference images",
            "",
            "Images (*.png *.jpg *.jpeg *.webp *.bmp)",
        )
        tags, accepted = QInputDialog.getText(
            self, "Reference tags", "Comma-separated tags"
        )
        if not accepted:
            return
        tags = [tag.strip() for tag in tags.split(",") if tag.strip()]
        for path in paths:
            if os.path.isfile(path):
                self.store.add(path, tags)
        self.refresh()

    def activate_license(self):
        key, accepted = QInputDialog.getText(self, "Support Pinwheel", "Gumroad receipt key")
        if not accepted:
            return
        _ok, message = activate(key)
        self.setWindowTitle("Pinwheel references — " + message)

    def edit_tags(self):
        if not is_licensed():
            self.setWindowTitle("Pinwheel references — license required")
            return
        reference = self._selected_reference()
        if not reference:
            return
        text, accepted = QInputDialog.getText(
            self,
            "Reference tags",
            "Comma-separated tags",
            text=", ".join(reference["tags"]),
        )
        if not accepted:
            return
        reference["tags"] = [tag.strip() for tag in text.split(",") if tag.strip()]
        self.store.save()
        self.refresh()

    def remove_selected(self):
        if not is_licensed():
            self.setWindowTitle("Pinwheel references — license required")
            return
        reference = self._selected_reference()
        if not reference:
            return
        choice = QMessageBox.question(
            self,
            "Remove reference",
            "Remove this item from Pinwheel? The original image file is kept.",
            QMessageBox.Yes | QMessageBox.No,
            QMessageBox.No,
        )
        if choice == QMessageBox.Yes:
            self.store.remove(reference["id"])
            self.refresh()

    def pop_out(self, _item=None):
        del _item
        if not is_licensed():
            self.setWindowTitle("Pinwheel references")
            return
        reference = self._selected_reference()
        document = krita.active_document()
        if not reference or not document:
            return
        image = QImage(reference["path"])
        if image.isNull():
            return
        try:
            krita.place_image_as_reference(document, image, "Pinwheel reference")
        except RuntimeError as error:
            self.setWindowTitle("Pinwheel references, error: " + str(error))
            return
        self.setWindowTitle(
            "Pinwheel references, added " + os.path.basename(reference["path"])
        )


krita.register_dock_widget("pinwheel_references", PinwheelDocker)
