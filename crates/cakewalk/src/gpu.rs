//! Optional WGPU shader brush source validation and module construction.
//!
//! Cakewalk deliberately accepts Vulkan-semantic GLSL 450. Naga parses that
//! source before WGPU sees it, so plugins receive an actionable compiler error.

use std::borrow::Cow;

pub const GLSL_BRUSH_STAGE: wgpu::naga::ShaderStage = wgpu::naga::ShaderStage::Compute;

pub fn validate_glsl(source: &str) -> Result<(), String> {
    let mut frontend = naga::front::glsl::Frontend::default();
    frontend
        .parse(&naga::front::glsl::Options::from(naga::ShaderStage::Compute), source)
        .map(|_| ())
        .map_err(|error| error.emit_to_string(source))
}

pub fn shader_module(device: &wgpu::Device, label: &str, source: &str) -> Result<wgpu::ShaderModule, String> {
    validate_glsl(source)?;
    Ok(device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Glsl {
            shader: Cow::Borrowed(source),
            stage: GLSL_BRUSH_STAGE,
            defines: &[],
        },
    }))
}
