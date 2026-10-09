# shadeproject

Uses MiDaS + onnxruntime/tract plus a pretrained neural network designed to directly predict Gaussian parameters from a single image, projecting "render maps"/3d, distorted grids to guide in shading and lighting. This plugin is designed to assist artists in creating realistic shading and lighting effects in their artwork by providing a reference for how light interacts with surfaces.

ShadeProject packages CUDA 12.8 PyTorch wheels. CUDA is required for
inference—there is no silent CPU fallback—so package using the same compatible
Python ABI as the target Krita build and run it on a supported NVIDIA driver.

MiDaS is downloaded through PyTorch Hub on its first inference. Depth maps are
cached by canvas content and model revision; auto-update checks the active
canvas every two seconds and reuses a cached result when pixels have not
changed. **Fine-tune from image + depth map** uses CUDA SGD with the active
canvas as its image input, saves an opt-in local checkpoint, and invalidates
base-model cache entries by giving the checkpoint its own cache revision.
