use anyhow::{anyhow, ensure, Context, Result};
use image::{Rgb, RgbImage};
use wgpu::util::DeviceExt;

fn adapter() -> Result<wgpu::Adapter> {
    pollster::block_on(
        wgpu::Instance::default().request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }),
    )
    .context("Geen geschikte GPU gevonden. Kies CPU-verwerking.")
}
pub fn description() -> Option<String> {
    adapter().ok().map(|adapter| {
        let info = adapter.get_info();
        format!("{} ({:?})", info.name, info.backend)
    })
}
pub fn upscale(image: &RgbImage) -> Result<RgbImage> {
    let adapter = adapter()?;
    let limits = adapter.limits();
    let output_size = image.width() as u64 * image.height() as u64 * 16;
    ensure!(
        output_size <= limits.max_storage_buffer_binding_size as u64
            && output_size <= limits.max_buffer_size,
        "Pagina te groot voor de GPU-buffer. Kies CPU."
    );
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("ComicGPU"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
        },
        None,
    ))?;
    let pixels: Vec<u32> = image
        .pixels()
        .map(|p| u32::from(p[0]) | (u32::from(p[1]) << 8) | (u32::from(p[2]) << 16))
        .collect();
    let input = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&pixels),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let dimensions = [image.width(), image.height(), 0, 0];
    let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&dimensions),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Bicubisch 2x"),
        source: wgpu::ShaderSource::Wgsl(include_str!("upscale.wgsl").into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader,
        entry_point: "main",
    });
    if let Some(error) = pollster::block_on(device.pop_error_scope()) {
        return Err(anyhow!("GPU-pipeline niet beschikbaar: {error}"));
    }
    let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params.as_entire_binding(),
            },
        ],
    });
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bindings, &[]);
        pass.dispatch_workgroups(
            (image.width() * 2).div_ceil(8),
            (image.height() * 2).div_ceil(8),
            1,
        );
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, output_size);
    queue.submit(Some(encoder.finish()));
    let slice = readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    receiver.recv().map_err(|e| anyhow!(e))??;
    let mapped = slice.get_mapped_range();
    let values: &[u32] = bytemuck::cast_slice(&mapped);
    let result = RgbImage::from_fn(image.width() * 2, image.height() * 2, |x, y| {
        let p = values[(y * image.width() * 2 + x) as usize];
        Rgb([p as u8, (p >> 8) as u8, (p >> 16) as u8])
    });
    drop(mapped);
    readback.unmap();
    Ok(result)
}
