//! Rendering GPU con wgpu: un'unica `Device` condivisa, una superficie per finestra,
//! uno shader per sfondo che disegna un triangolo a schermo intero.

use crate::config::log;
use crate::wallpapers::{self, Wallpaper};
use std::collections::HashMap;
use std::sync::Arc;
use winit::window::Window;

/// Parametri passati allo shader. Deve combaciare con `struct Params` in prelude.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Params {
    /// Dimensione della finestra in pixel fisici.
    pub res: [f32; 2],
    /// Posizione della finestra dentro la "tela" (≠ 0 solo in modalità estesa).
    pub offset: [f32; 2],
    /// Dimensione della tela: lo schermo, oppure l'unione di tutti gli schermi.
    pub canvas: [f32; 2],
    /// Tempo condiviso da tutte le finestre, in secondi.
    pub time: f32,
    /// Secondi dall'ultimo cambio di sfondo (per l'animazione di apertura).
    pub since: f32,
    pub logo_on: f32,
    /// Rapporto tra pixel disegnati e pixel della finestra (1 = risoluzione piena).
    pub scale: f32,
    pub _pad: [f32; 2],
}

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,
    bgl: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    logo_view: wgpu::TextureView,
    pipelines: HashMap<(String, wgpu::TextureFormat), Option<wgpu::RenderPipeline>>,
}

pub struct Target {
    pub format: wgpu::TextureFormat,
    pub scale: f32,
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub uniforms: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

pub fn new_instance() -> wgpu::Instance {
    // Un solo backend per sistema (DirectX 12 / Metal): non serve il "display handle".
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = if cfg!(windows) {
        wgpu::Backends::DX12
    } else if cfg!(target_os = "macos") {
        wgpu::Backends::METAL
    } else {
        wgpu::Backends::VULKAN
    };
    // Nelle build di rilascio nessun livello di validazione o debug, che occupa memoria.
    if !cfg!(debug_assertions) {
        desc.flags = wgpu::InstanceFlags::empty();
    }
    wgpu::Instance::new(desc)
}

impl Gpu {
    /// Crea il dispositivo GPU. `surface` serve a scegliere una scheda compatibile con le finestre.
    pub fn new(instance: wgpu::Instance, surface: Option<&wgpu::Surface<'_>>) -> Result<Self, String> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            // Preferisce la GPU integrata: consuma meno ed è più che sufficiente.
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: surface,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .map_err(|e| format!("nessuna GPU compatibile: {e}"))?;
        log(format!("GPU: {:?}", adapter.get_info()));

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("devlogica"),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            // Blocchi di memoria piccoli: le nostre risorse sono minuscole, non serve riservare
            // i grandi blocchi pensati per i giochi.
            memory_hints: wgpu::MemoryHints::Manual { suballocated_device_memory_block_size: (256 << 10)..(4 << 20) },
            ..Default::default()
        }))
        .map_err(|e| format!("impossibile aprire la GPU: {e}"))?;

        // Si lavora in spazio sRGB "grezzo" (come un canvas del browser), quindi formato non-sRGB.
        let format = match surface {
            Some(s) => {
                let caps = s.get_capabilities(&adapter);
                caps.formats
                    .iter()
                    .copied()
                    .find(|f| !f.is_srgb())
                    .unwrap_or(caps.formats[0])
            }
            None => wgpu::TextureFormat::Rgba8Unorm,
        };

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("params"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("logo"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let logo_view = upload_logo(&device, &queue);

        Ok(Self { instance, adapter, device, queue, format, bgl, layout, sampler, logo_view, pipelines: HashMap::new() })
    }

    /// Compila (una sola volta) lo shader di uno sfondo. `None` se lo shader contiene errori.
    pub fn pipeline(&mut self, w: &Wallpaper, format: wgpu::TextureFormat) -> Option<&wgpu::RenderPipeline> {
        let key = (w.id.clone(), format);
        if !self.pipelines.contains_key(&key) {
            let p = self.compile(w, format);
            self.pipelines.insert(key.clone(), p);
        }
        self.pipelines.get(&key).and_then(|p| p.as_ref())
    }

    /// Formato di disegno per una superficie: sempre non-sRGB, come un canvas del browser.
    pub fn surface_format(&self, surface: &wgpu::Surface<'_>) -> wgpu::TextureFormat {
        let caps = surface.get_capabilities(&self.adapter);
        caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0])
    }

    /// Riempie una superficie con un colore (usato dal bordo rosso degli avvisi).
    pub fn clear(&self, t: &mut Target, color: wgpu::Color) {
        let frame = match t.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            _ => {
                t.surface.configure(&self.device, &t.config);
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(color), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        self.queue.submit([enc.finish()]);
        self.queue.present(frame);
    }

    pub fn clear_pipelines(&mut self) {
        self.pipelines.clear();
    }

    /// Libera gli shader degli sfondi che non sono più in uso.
    pub fn retain_pipelines(&mut self, in_use: &[String]) {
        self.pipelines.retain(|(id, _), _| in_use.contains(id));
    }

    fn compile(&self, w: &Wallpaper, format: wgpu::TextureFormat) -> Option<wgpu::RenderPipeline> {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&w.id),
            source: wgpu::ShaderSource::Wgsl(wallpapers::full_source(w).into()),
        });
        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&w.id),
            layout: Some(&self.layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        if let Some(err) = pollster::block_on(scope.pop()) {
            log(format!("Errore nello sfondo '{}': {err}", w.name));
            return None;
        }
        Some(pipeline)
    }

    fn bind_group(&self, uniforms: &wgpu::Buffer) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("params"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.logo_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        })
    }

    fn uniform_buffer(&self) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub fn make_target(&self, window: Arc<Window>, surface: wgpu::Surface<'static>, scale: f32) -> Target {
        let size = window.inner_size();
        let (w, h) = scaled(size.width, size.height, scale);
        let caps = surface.get_capabilities(&self.adapter);
        let format = self.surface_format(&surface);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: w,
            height: h,
            present_mode: wgpu::PresentMode::AutoVsync,
            // Due buffer invece di tre: a 30 fps non serve altro, e ogni buffer 4K pesa ~33 MB.
            desired_maximum_frame_latency: 1,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&self.device, &config);
        let uniforms = self.uniform_buffer();
        let bind_group = self.bind_group(&uniforms);
        Target { format, scale, window, surface, config, uniforms, bind_group }
    }

    pub fn resize(&self, t: &mut Target, w: u32, h: u32) {
        let (w, h) = scaled(w, h, t.scale);
        t.config.width = w;
        t.config.height = h;
        t.surface.configure(&self.device, &t.config);
    }

    pub fn draw(&mut self, t: &mut Target, w: &Wallpaper, params: Params) {
        let frame = match t.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            _ => {
                // Superficie persa o obsoleta (cambio risoluzione, monitor scollegato...): la si ricrea.
                t.surface.configure(&self.device, &t.config);
                return;
            }
        };
        self.queue.write_buffer(&t.uniforms, 0, bytemuck::bytes_of(&params));
        let view = frame.texture.create_view(&Default::default());
        let Some(pipeline) = self.pipeline(w, t.format) else {
            // Sfondo con errori: si mostra un fondo neutro invece di un'immagine bloccata.
            let mut enc = self.device.create_command_encoder(&Default::default());
            clear_pass(&mut enc, &view);
            self.queue.submit([enc.finish()]);
            self.queue.present(frame);
            return;
        };
        let pipeline = pipeline.clone();
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = begin_pass(&mut enc, &view);
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &t.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([enc.finish()]);
        self.queue.present(frame);
    }

    /// Disegna uno sfondo su una texture fuori schermo e restituisce i pixel RGBA.
    /// Usato per esportare immagini statiche (opzione --snapshot).
    pub fn render_offscreen(&mut self, w: &Wallpaper, params: Params) -> Option<Vec<u8>> {
        let (width, height) = scaled(params.res[0] as u32, params.res[1] as u32, params.scale);
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let uniforms = self.uniform_buffer();
        let bind_group = self.bind_group(&uniforms);
        self.queue.write_buffer(&uniforms, 0, bytemuck::bytes_of(&params));
        let pipeline = self.pipeline(w, self.format)?.clone();

        let row = (width * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = begin_pass(&mut enc, &view);
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(height) },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let data = slice.get_mapped_range().ok()?;
        let bgra = matches!(self.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            let line = &data[(y * row) as usize..(y * row + width * 4) as usize];
            for px in line.chunks_exact(4) {
                if bgra { out.extend_from_slice(&[px[2], px[1], px[0], 255]) } else { out.extend_from_slice(&[px[0], px[1], px[2], 255]) }
            }
        }
        Some(out)
    }
}

/// Dimensione della superficie di disegno: il sistema la ingrandisce da solo alla finestra.
fn scaled(w: u32, h: u32, scale: f32) -> (u32, u32) {
    (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1))
}

fn begin_pass<'a>(enc: &'a mut wgpu::CommandEncoder, view: &'a wgpu::TextureView) -> wgpu::RenderPass<'a> {
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

fn clear_pass(enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
    let _ = begin_pass(enc, view);
}

/// Carica il logo (PNG con wordmark bianco) come texture con mipmap, così resta nitido a ogni dimensione.
fn upload_logo(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let img = image::load_from_memory(include_bytes!("../assets/logo.png")).expect("logo").to_rgba8();
    let (w, h) = img.dimensions();
    let levels = (w.max(h) as f32).log2().floor() as u32 + 1;
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("logo"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    // Alfa premoltiplicato prima di ridurre, per non avere aloni scuri ai bordi.
    let mut level = image::RgbaImage::from_fn(w, h, |x, y| {
        let p = img.get_pixel(x, y).0;
        let a = p[3] as u32;
        image::Rgba([(p[0] as u32 * a / 255) as u8, (p[1] as u32 * a / 255) as u8, (p[2] as u32 * a / 255) as u8, p[3]])
    });
    for mip in 0..levels {
        let (lw, lh) = level.dimensions();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: mip, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &level,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(lw * 4), rows_per_image: Some(lh) },
            wgpu::Extent3d { width: lw, height: lh, depth_or_array_layers: 1 },
        );
        if lw == 1 && lh == 1 {
            break;
        }
        level = image::imageops::resize(&level, (lw / 2).max(1), (lh / 2).max(1), image::imageops::FilterType::Triangle);
    }
    tex.create_view(&Default::default())
}
