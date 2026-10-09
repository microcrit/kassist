import math
from pathlib import Path

from krita import DockWidget, DockWidgetFactory, DockWidgetFactoryBase, Krita
from PyQt5.QtCore import QPointF
from PyQt5.QtGui import QColor, QPainter, QPen
from PyQt5.QtWidgets import QDialog, QInputDialog, QLabel, QVBoxLayout, QWidget

from .kassist_components import Panel
from .license import activate, is_licensed


class MeshData:
    def __init__(self, vertices, faces, name):
        self.vertices = vertices
        self.faces = faces
        self.name = name

    @classmethod
    def load(cls, path):
        import trimesh

        scene = trimesh.load(path, force="scene", process=False)
        mesh = scene.dump(concatenate=True)
        if mesh.vertices.size == 0 or mesh.faces.size == 0:
            raise RuntimeError("the scene has no triangle mesh data")
        return cls(
            mesh.vertices.astype("float32", copy=False), mesh.faces, Path(path).name
        )


class Wireframe(QWidget):
    def __init__(self, mesh, pose, parent=None):
        super().__init__(parent)
        self.mesh, self.pose = mesh, pose
        self.edges = []
        self.setMinimumSize(640, 480)
        self._project()

    def _project(self):
        import numpy as np

        vertices = self.mesh.vertices.copy()
        vertices -= vertices.mean(axis=0, keepdims=True)
        radius = float(np.max(np.linalg.norm(vertices, axis=1))) or 1.0
        vertices /= radius
        yaw, pitch, roll = [
            math.radians(self.pose[key]) for key in ("yaw", "pitch", "roll")
        ]
        cy, sy = math.cos(yaw), math.sin(yaw)
        cp, sp = math.cos(pitch), math.sin(pitch)
        cr, sr = math.cos(roll), math.sin(roll)
        rotation = np.array(
            [
                [cr * cy - sr * sp * sy, -sr * cp, cr * sy + sr * sp * cy],
                [sr * cy + cr * sp * sy, cr * cp, sr * sy - cr * sp * cy],
                [-cp * sy, sp, cp * cy],
            ],
            dtype="float32",
        )
        vertices = vertices @ rotation.T * self.pose["scale"]
        depth = vertices[:, 2] + 3.0
        projected = vertices[:, :2] / depth[:, None]
        budget = int(self.pose["budget"])
        stride = max(1, len(self.mesh.faces) // max(1, budget))
        edges = []
        for face in self.mesh.faces[::stride]:
            points = projected[face]
            edges.extend(
                ((points[0], points[1]), (points[1], points[2]), (points[2], points[0]))
            )
        self.edges = edges

    def paintEvent(self, event):
        del event
        painter = QPainter(self)
        painter.fillRect(self.rect(), QColor("#20242a"))
        painter.setRenderHint(QPainter.Antialiasing, False)
        painter.setPen(QPen(QColor("#82c7ff"), 1))
        scale = min(self.width(), self.height()) * 1.7
        center_x, center_y = self.width() / 2.0, self.height() / 2.0
        for first, second in self.edges:
            painter.drawLine(
                QPointF(center_x + first[0] * scale, center_y - first[1] * scale),
                QPointF(center_x + second[0] * scale, center_y - second[1] * scale),
            )
        painter.end()


class ModelPreviewDocker(DockWidget):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Model Preview")
        self.state = {
            "path": "",
            "yaw": 25.0,
            "pitch": -12.0,
            "roll": 0.0,
            "scale": 1.0,
            "budget": 8000.0,
            "makehuman_path": str(Path.home() / "kassist-character.mhm"),
            "gender": 0.5,
            "age": 0.5,
            "muscle": 0.5,
            "weight": 0.5,
        }
        content = QWidget(self)
        layout = QVBoxLayout(content)
        self.status = QLabel("Choose a GLB, glTF, or FBX model to preview.")
        layout.addWidget(self.status)
        Panel("Model Preview", self._dispatch).file_picker(
            "Model",
            self.state["path"],
            "Choose a GLB, glTF, or FBX model",
            "open",
            "path",
        ).row(
            lambda ui: ui.slider("Yaw", 25.0, -180.0, 180.0, 0, "yaw").slider(
                "Pitch", -12.0, -90.0, 90.0, 0, "pitch"
            )
        ).slider("Roll", 0.0, -180.0, 180.0, 0, "roll").number(
            "Scale", 1.0, 0.1, 5.0, 0.1, 1, "scale"
        ).number(
            "Preview face budget", 8000.0, 500.0, 50000.0, 500.0, 0, "budget"
        ).separator().text("MakeHuman profile generator").file_picker(
            "Output .mhm",
            self.state["makehuman_path"],
            "Save MakeHuman profile",
            "save",
            "makehuman_path",
        ).row(
            lambda ui: ui.slider("Gender", 0.5, 0.0, 1.0, 2, "gender").slider(
                "Age", 0.5, 0.0, 1.0, 2, "age"
            )
        ).row(
            lambda ui: ui.slider("Muscle", 0.5, 0.0, 1.0, 2, "muscle").slider(
                "Weight", 0.5, 0.0, 1.0, 2, "weight"
            )
        ).row(
            lambda ui: ui.button("Open Qt preview", "preview").button(
                "Write MakeHuman profile", "makehuman"
            )
        ).button("Support with Gumroad", "activate").mount(layout)
        self.setWidget(content)

    def canvasChanged(self, canvas):
        self.canvas = canvas
        if canvas is None:
            self.status.setText("Open a document to use canvas-linked model references.")

    def _dispatch(self, action, value):
        if action in self.state:
            self.state[action] = value
        elif action == "preview":
            self.preview()
        elif action == "makehuman":
            self.write_makehuman()
        elif action == "activate":
            key, accepted = QInputDialog.getText(
                self, "Activate Model Preview", "License key"
            )
            if accepted:
                _ok, message = activate(key)
                self.status.setText(message)
        return True

    def preview(self):
        if not is_licensed():
            self.status.setText("License required (or compile with --dev).")
            return
        if not self.state["path"]:
            self.status.setText("Choose a model file first.")
            return
        try:
            mesh = MeshData.load(self.state["path"])
            dialog = QDialog(self)
            dialog.setWindowTitle(f"{mesh.name} — wireframe preview")
            layout = QVBoxLayout(dialog)
            layout.addWidget(Wireframe(mesh, self.state, dialog))
            dialog.resize(800, 620)
            dialog.exec_()
            self.status.setText(f"Previewed {len(mesh.faces)} triangles.")
        except Exception as error:
            self.status.setText(f"Model import failed: {error}")

    def write_makehuman(self):
        if not is_licensed():
            self.status.setText("License required (or compile with --dev).")
            return
        path = Path(self.state["makehuman_path"]).expanduser()
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            profile = (
                "# Generated by kassist ModelPreview\n"
                + "\n".join(
                    f"modifier {name} {self.state[key]}"
                    for name, key in (
                        ("macrodetails/Gender", "gender"),
                        ("macrodetails/Age", "age"),
                        ("macrodetails-universal/Muscle", "muscle"),
                        ("macrodetails-universal/Weight", "weight"),
                    )
                )
                + "\n"
            )
            path.write_text(profile, encoding="utf-8")
            self.status.setText(f"Wrote MakeHuman profile {path.name}.")
        except OSError as error:
            self.status.setText(f"Could not write MakeHuman profile: {error}")


Krita.instance().addDockWidgetFactory(
    DockWidgetFactory(
        "modelpreview", DockWidgetFactoryBase.DockRight, ModelPreviewDocker
    )
)
