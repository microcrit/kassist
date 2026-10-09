from pathlib import Path


def torch_module():
    import torch

    return torch


def require_cuda():
    torch = torch_module()
    if not torch.version.cuda:
        raise RuntimeError("the embedded PyTorch build has no CUDA runtime")
    if not torch.cuda.is_available():
        raise RuntimeError("no usable NVIDIA CUDA device was found")
    return torch, torch.device("cuda")


def cache_directory(*parts):
    root = Path.home() / ".cache" / "kassist" / "torch"
    path = root.joinpath(*parts)
    path.mkdir(parents=True, exist_ok=True)
    return path


def inference(model, input_tensor):
    """Run a model in CUDA inference mode and return its output."""
    torch, device = require_cuda()
    model.to(device)
    model.eval()
    with torch.inference_mode():
        return model(input_tensor.to(device))


def sgd(parameters, learning_rate=1e-4, momentum=0.9):
    """Create the standard SGD optimizer used by kassist fine-tuning flows."""
    torch, _device = require_cuda()
    return torch.optim.SGD(
        parameters, lr=float(learning_rate), momentum=float(momentum)
    )
