use vulkano::command_buffer::{DynamicState};
use vulkano::device::{Device, DeviceExtensions, Queue};
use vulkano::format::Format;
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract};
use vulkano::image::SwapchainImage;
use vulkano::instance::Instance;
use vulkano::instance::PhysicalDevice;
use vulkano::pipeline::viewport::Viewport;
use vulkano::swapchain::{PresentMode, Surface, SurfaceTransform, Swapchain, ColorSpace, FullscreenExclusive};

use vulkano_win::VkSurfaceBuild;
use winit::window::{WindowBuilder, Window};
use winit::event_loop::{EventLoop};

use std::sync::Arc;

pub fn vulkan_init() -> (Arc<Device>, Arc<dyn RenderPassAbstract + Send + Sync>, Vec<Arc<SwapchainImage<Window>>>, EventLoop<()>, Arc<Surface<Window>>, Arc<Swapchain<Window>>, Arc<Queue>) {
    let required_extensions = vulkano_win::required_extensions();
    let instance = Instance::new(None, &required_extensions, None)
        .unwrap();
    let physical = PhysicalDevice::enumerate(&instance)
        .next()
        .unwrap();
    println!("Using device: {} (type: {:?})", physical.name(), physical.ty());

    let event_loop = EventLoop::new();
    let surface = WindowBuilder::new()
        .build_vk_surface(&event_loop, instance.clone())
        .unwrap();

    let (device, queue) = create_device_and_queue(physical.clone(), &surface);

    // i3wm reports min and max image extents that are identical. This is a sort of workaround for me
    // Use surface.window().inner_size().into() if it doesn't panic for you
    let dimensions: [u32; 2] = surface.capabilities(device.physical_device())
        .unwrap()
        .min_image_extent;

    let (swapchain, images) = create_swapchain(
        &queue,
        &surface,
        &device,
        dimensions,
    );

    let render_pass = create_render_pass(&device, swapchain.format());
    (device, render_pass, images, event_loop, surface, swapchain, queue)
}

pub fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    dynamic_state: &mut DynamicState,
) -> Vec<Arc<dyn FramebufferAbstract + Send + Sync>> {
    let dimensions = images[0].dimensions();

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [dimensions[0] as f32, dimensions[1] as f32],
        depth_range: 0.0..1.0,
    };

    dynamic_state.viewports = Some(vec!(viewport));

    images.iter()
        .map(|image| {
            Arc::new(
                Framebuffer::start(render_pass.clone())
                    .add(image.clone())
                    .unwrap()
                    .build()
                    .unwrap()
            ) as Arc<dyn FramebufferAbstract + Send + Sync>
        })
        .collect::<Vec<_>>()
}

fn create_render_pass(device: &Arc<Device>, format: Format) -> Arc<dyn RenderPassAbstract + Send + Sync> {
    Arc::new(
        vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: format,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        )
            .unwrap()
    )
}

fn create_device_and_queue(physical: PhysicalDevice, surface: &Arc<Surface<Window>>)
                           -> (Arc<Device>, Arc<Queue>) {
    let queue_family = physical.queue_families()
        .find(|&q| {
            q.supports_graphics() && surface.is_supported(q).unwrap_or(false)
        })
        .unwrap();

    let device_extensions = DeviceExtensions {
        khr_swapchain: true,
        ..DeviceExtensions::none()
    };

    let (device, mut queues) = Device::new(
        physical,
        physical.supported_features(),
        &device_extensions,
        [(queue_family, 0.5)].iter()
            .cloned(),
    )
        .unwrap();

    let queue = queues.next()
        .unwrap();
    (device, queue)
}

fn create_swapchain(queue: &Arc<Queue>, surface: &Arc<Surface<Window>>, device: &Arc<Device>, dimensions: [u32; 2])
                    -> (Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>) {
    let dev = device.clone();
    let caps = surface.capabilities(dev.physical_device().clone())
        .unwrap();
    let usage = caps.supported_usage_flags;
    let alpha = caps.supported_composite_alpha
        .iter()
        .next()
        .unwrap();
    let format = caps.supported_formats[0].0;

    Swapchain::new(
        dev,
        surface.clone(),
        caps.min_image_count,
        format,
        dimensions,
        1,
        usage,
        queue,
        SurfaceTransform::Identity,
        alpha,
        PresentMode::Fifo,
        FullscreenExclusive::Default,
        true,
        ColorSpace::SrgbNonLinear,
    )
        .unwrap()
}