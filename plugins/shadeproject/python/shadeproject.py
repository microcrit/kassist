import hashlib
import os
import tempfile
from pathlib import Path

from krita import DockWidget, InfoObject
from PyQt5.QtCore import QObject, QThread, QTimer, pyqtSignal, pyqtSlot
from PyQt5.QtGui import QImage
from PyQt5.QtWidgets import (
    QFileDialog,
    QInputDialog,
    QLabel,
    QVBoxLayout,
    QWidget,
)

from . import kassist_krita as krita
from . import kassist_torch as torch_api
from .kassist_components import Panel as ComponentPanel
from .license import activate, is_licensed


class MidasCache:
    def __init__(self):
        self.model = self.transform = self.last_key = self.last_depth = None
        self.device = None
        self.cache_root = Path.home() / ".cache" / "kassist"
        self.depth_cache = self.cache_root / "depth"
        self.depth_cache.mkdir(parents=True, exist_ok=True)
        self.checkpoint = self.cache_root / "midas-small-finetuned.pt"
        self.model_revision = self._checkpoint_revision()

    def _checkpoint_revision(self):
        if self.checkpoint.is_file():
            return f"custom-{self.checkpoint.stat().st_mtime_ns}"
        return "base"

    def load(self):
        if self.model is not None:
            return
        import torch

        torch, self.device = torch_api.require_cuda()

        torch.hub.set_dir(str(self.cache_root / "torch"))
        self.model = torch.hub.load("isl-org/MiDaS", "MiDaS_small", trust_repo="check")
        transforms = torch.hub.load("isl-org/MiDaS", "transforms", trust_repo="check")
        self.transform = transforms.small_transform
        self.model.to(self.device)
        if self.checkpoint.is_file():
            checkpoint = torch.load(
                str(self.checkpoint), map_location=self.device, weights_only=True
            )
            self.model.load_state_dict(checkpoint["model"])
            self.model_revision = self._checkpoint_revision()
        self.model.eval()

    def infer(self, image_path):
        import cv2
        import numpy as np
        import torch

        source = cv2.imread(image_path)
        if source is None:
            raise ValueError("Krita could not export an image for depth inference")
        key = hashlib.sha256(source.tobytes()).hexdigest()
        if key == self.last_key:
            return self.last_depth
        cached = self.depth_cache / (self.model_revision + "-" + key + ".npy")
        if cached.is_file():
            depth = np.load(str(cached), allow_pickle=False)
            self.last_key, self.last_depth = key, depth
            return depth
        self.load()
        rgb = cv2.cvtColor(source, cv2.COLOR_BGR2RGB)
        with torch.inference_mode():
            prediction = self.model(self.transform(rgb).to(self.device))
            depth = (
                torch.nn.functional.interpolate(
                    prediction.unsqueeze(1),
                    size=rgb.shape[:2],
                    mode="bicubic",
                    align_corners=False,
                )
                .squeeze()
                .cpu()
                .numpy()
            )
        np.save(str(cached), depth, allow_pickle=False)
        self.last_key, self.last_depth = key, depth
        return depth

    def fine_tune_paths(self, image_path, depth_path, epochs=1, learning_rate=1e-4):
        import cv2
        import numpy as np
        import torch

        self.load()
        source = cv2.imread(image_path)
        target = cv2.imread(depth_path, cv2.IMREAD_UNCHANGED)
        if source is None or target is None:
            raise ValueError("choose readable image and depth-map files")
        if target.ndim == 3:
            color = cv2.COLOR_BGRA2GRAY if target.shape[2] == 4 else cv2.COLOR_BGR2GRAY
            target = cv2.cvtColor(target, color)
        target = target.astype(np.float32)
        minimum, maximum = float(np.min(target)), float(np.max(target))
        if maximum - minimum <= 1e-6:
            raise ValueError("the depth map must contain more than one depth value")
        target = (target - minimum) / (maximum - minimum)
        rgb = cv2.cvtColor(source, cv2.COLOR_BGR2RGB)
        image = self.transform(rgb).to(self.device)
        target = torch.from_numpy(target).unsqueeze(0).unsqueeze(0).to(self.device)
        optimizer = torch.optim.SGD(
            self.model.parameters(), lr=learning_rate, momentum=0.9
        )
        self.model.train()
        loss_value = 0.0
        for _ in range(epochs):
            optimizer.zero_grad()
            prediction = self.model(image)
            expected = torch.nn.functional.interpolate(
                target, size=prediction.shape[-2:], mode="bilinear", align_corners=False
            ).squeeze(1)
            loss = torch.nn.functional.mse_loss(prediction, expected)
            loss.backward()
            optimizer.step()
            loss_value = float(loss.detach().cpu())
        self.model.eval()
        torch.save(
            {
                "model": {
                    name: value.detach().cpu()
                    for name, value in self.model.state_dict().items()
                }
            },
            str(self.checkpoint),
        )
        self.model_revision = self._checkpoint_revision()
        self.last_key = self.last_depth = None
        return loss_value


class ShadeWorker(QObject):
    depth_ready = pyqtSignal(object)
    training_ready = pyqtSignal(float)
    failed = pyqtSignal(str)
    finished = pyqtSignal()

    def __init__(self):
        super().__init__()
        self.cache = None

    def _cache(self):
        if self.cache is None:
            self.cache = MidasCache()
        return self.cache

    @pyqtSlot(str)
    def infer(self, image_path):
        try:
            self.depth_ready.emit(self._cache().infer(image_path))
        except Exception as error:
            self.failed.emit(str(error))
        finally:
            if os.path.exists(image_path):
                os.unlink(image_path)
            self.finished.emit()

    @pyqtSlot(str, str, int)
    def fine_tune(self, image_path, depth_path, epochs):
        try:
            loss = self._cache().fine_tune_paths(image_path, depth_path, epochs)
            self.training_ready.emit(float(loss))
        except Exception as error:
            self.failed.emit(str(error))
        finally:
            if os.path.exists(image_path):
                os.unlink(image_path)
            self.finished.emit()


class ShadeProjectDocker(DockWidget):
    inference_requested = pyqtSignal(str)
    training_requested = pyqtSignal(str, str, int)

    def __init__(self):
        super().__init__()
        self.setWindowTitle("Shade Project")
        self.status = QLabel("Update depth manually or enable cached auto-update.")
        content = QWidget(self)
        layout = QVBoxLayout(content)
        layout.addWidget(self.status)
        ComponentPanel("Shade Project", self._dispatch_action).row(
            lambda ui: ui.button("Update depth reference", "refresh").button(
                "Fine-tune from image + depth map", "fine_tune"
            )
        ).separator().toggle(
            "Auto-update every 2 seconds while drawing", False, "auto_update"
        ).button("Support with Gumroad", "activate").mount(layout)
        self.setWidget(content)
        self.timer = QTimer(self)
        self.timer.setSingleShot(True)
        self.timer.setInterval(700)
        self.timer.timeout.connect(self.refresh)
        self.auto_timer = QTimer(self)
        self.auto_timer.setInterval(2000)
        self.auto_timer.timeout.connect(self._auto_refresh)
        self._auto_licensed = False
        self._refreshing = False
        self._pending_document = None
        self.worker_thread = QThread(self)
        self.worker = ShadeWorker()
        self.worker.moveToThread(self.worker_thread)
        self.inference_requested.connect(self.worker.infer)
        self.training_requested.connect(self.worker.fine_tune)
        self.worker.depth_ready.connect(self._write_depth_result)
        self.worker.training_ready.connect(self._write_training_result)
        self.worker.failed.connect(self._write_worker_error)
        self.worker.finished.connect(self._finish_worker_job)
        self.worker_thread.finished.connect(self.worker.deleteLater)
        self.worker_thread.start()

    def _dispatch_action(self, action, value=None):
        if action == "refresh":
            self.refresh()
        elif action == "fine_tune":
            self.fine_tune()
        elif action == "activate":
            self.activate_license()
        elif action == "auto_update":
            self.set_auto_update(value)

    def canvasChanged(self, canvas):
        if canvas:
            self.timer.start()

    def set_auto_update(self, enabled):
        if not enabled:
            self.auto_timer.stop()
            return True
        self._auto_licensed = is_licensed()
        if not self._auto_licensed:
            self.status.setText("License required for auto-update.")
            return False
        self.auto_timer.start()
        self.status.setText("Auto-update checks the canvas every 2 seconds.")
        return True

    def _auto_refresh(self):
        if self._auto_licensed and not self._refreshing:
            self.refresh(check_license=False)

    def refresh(self, check_license=True):
        if self._refreshing:
            return
        if check_license and not is_licensed():
            self.status.setText("License required.")
            return
        document = krita.active_document()
        if not document:
            self.status.setText("Open a document before generating a depth reference.")
            return
        self._refreshing = True
        try:
            image_path = self._export_current_canvas(document)
            self._pending_document = document
            self.status.setText("CUDA depth inference is running in the background…")
            self.inference_requested.emit(image_path)
        except Exception as error:
            self._refreshing = False
            self.status.setText(f"MiDaS unavailable: {error}")

    def fine_tune(self):
        if not is_licensed():
            self.status.setText("License required.")
            return
        document = krita.active_document()
        if not document:
            self.status.setText("Open the artwork to use as the training image.")
            return
        depth_path, _ = QFileDialog.getOpenFileName(
            self,
            "Choose a grayscale depth-map target",
            "",
            "Images (*.png *.jpg *.jpeg *.tif *.tiff *.bmp)",
        )
        if not depth_path:
            return
        epochs, accepted = QInputDialog.getInt(
            self, "Fine-tune MiDaS", "SGD epochs", value=1, min=1, max=100
        )
        if not accepted:
            return
        self._refreshing = True
        self.status.setText("Fine-tuning CUDA MiDaS in the background…")
        try:
            image_path = self._export_current_canvas(document)
            self.training_requested.emit(image_path, depth_path, epochs)
        except Exception as error:
            self._refreshing = False
            self.status.setText(f"Fine-tuning failed: {error}")

    def _write_depth_result(self, depth):
        document = self._pending_document
        if document is None:
            return
        try:
            self._write_depth_layer(document, depth)
            self.status.setText(
                f"CUDA depth reference updated: {depth.shape[1]} × {depth.shape[0]}"
            )
        except Exception as error:
            self.status.setText(f"Could not write CUDA depth reference: {error}")

    def _write_training_result(self, loss):
        self.status.setText(f"Saved local MiDaS checkpoint; final SGD loss {loss:.5f}.")

    def _write_worker_error(self, error):
        self.status.setText(f"CUDA MiDaS job failed: {error}")

    def _finish_worker_job(self):
        self._pending_document = None
        self._refreshing = False

    def closeEvent(self, event):
        if self._refreshing:
            self.status.setText("A CUDA job is still running; wait for it to finish before closing Shade Project.")
            event.ignore()
            return
        self.auto_timer.stop()
        self.timer.stop()
        self.worker_thread.quit()
        self.worker_thread.wait(1000)
        super().closeEvent(event)

    def activate_license(self):
        key, accepted = QInputDialog.getText(
            self, "Activate ShadeProject", "License key"
        )
        if not accepted:
            return
        _ok, message = activate(key)
        self.status.setText(message)

    @staticmethod
    def _export_current_canvas(document):
        root = Path.home() / ".cache" / "kassist" / "exports"
        root.mkdir(parents=True, exist_ok=True)
        handle = tempfile.NamedTemporaryFile(suffix=".png", dir=str(root), delete=False)
        handle.close()
        if not document.exportImage(handle.name, InfoObject()):
            os.unlink(handle.name)
            raise RuntimeError("Krita could not export the active canvas")
        return handle.name

    @staticmethod
    def _depth_layer(document):
        return krita.find_node(document, "ShadeProject Depth")

    def _write_depth_layer(self, document, depth):
        import numpy as np

        minimum, maximum = float(np.min(depth)), float(np.max(depth))
        normalized = (depth - minimum) / max(maximum - minimum, 1e-6)
        grayscale = np.uint8((1.0 - normalized) * 255.0)
        rgba = np.empty((grayscale.shape[0], grayscale.shape[1], 4), dtype=np.uint8)
        rgba[..., 0] = grayscale
        rgba[..., 1] = grayscale
        rgba[..., 2] = grayscale
        rgba[..., 3] = 180
        image = QImage(
            rgba.data,
            grayscale.shape[1],
            grayscale.shape[0],
            grayscale.shape[1] * 4,
            QImage.Format_RGBA8888,
        ).copy()
        layer = self._depth_layer(document)
        if not layer:
            layer = krita.add_paint_layer(document, "ShadeProject Depth")
        krita.fill_layer_with_image(layer, image, document)
        layer.setOpacity(180)
        document.refreshProjection()


krita.register_dock_widget("shadeproject", ShadeProjectDocker)
