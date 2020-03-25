mod vulkan;

use vulkano::buffer::{CpuAccessibleBuffer, BufferUsage, CpuBufferPool};
use std::sync::Arc;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::framebuffer::Subpass;
use vulkano::command_buffer::{DynamicState, AutoCommandBufferBuilder};
use vulkano::sync::{self, GpuFuture, FlushError};
use winit::event::{Event, WindowEvent};
use winit::event_loop::ControlFlow;
use vulkano::swapchain::{self, SwapchainCreationError, AcquireError};
use vulkan::initialization::{vulkan_init, window_size_dependent_setup};
use std::time::Instant;
use cgmath::{Matrix3, Matrix4, Rad};
use vulkano::descriptor::PipelineLayoutAbstract;
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;

#[derive(Default, Debug, Clone)]
struct Vertex {
    position: [f32; 2]
}

vulkano::impl_vertex!(Vertex, position);

fn main() {
    let (
        device,
        render_pass,
        images,
        event_loop,
        surface,
        mut swapchain,
        queue
    ) = vulkan_init();

    let vertex_buffer = CpuAccessibleBuffer::from_iter(
        device.clone(),
        BufferUsage::all(),
        false,
        [
            Vertex { position: [-0.5, -0.25] },
            Vertex { position: [0.0, 0.5] },
            Vertex { position: [0.25, -0.1] }
        ]
            .iter()
            .cloned(),
    )
        .unwrap();

    let vs = vs::Shader::load(device.clone()).unwrap();
    let fs = fs::Shader::load(device.clone()).unwrap();
    let uniform_buffer = CpuBufferPool::<vs::ty::Data>::new(
        device.clone(),
        BufferUsage::all()
    );

    let pipeline = Arc::new(
        GraphicsPipeline::start()
            .vertex_input_single_buffer()
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ())
            .render_pass(
                Subpass::from(
                    render_pass.clone(),
                    0)
                    .unwrap()
            )
            .build(device.clone())
            .unwrap()
    );

    let mut dynamic_state = DynamicState {
        line_width: None,
        viewports: None,
        scissors: None,
        compare_mask: None,
        write_mask: None,
        reference: None,
    };

    let mut framebuffers = window_size_dependent_setup(
        &images,
        render_pass.clone(),
        &mut dynamic_state,
    );

    let mut recreate_swapchain = false;
    let mut previous_frame_end = Some(
        Box::new(
            sync::now(device.clone())
        ) as Box<dyn GpuFuture>
    );
    let rotation_duration = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
                recreate_swapchain = true;
            }
            Event::RedrawEventsCleared => {
                previous_frame_end.as_mut()
                    .unwrap()
                    .cleanup_finished();

                if recreate_swapchain {
                    let dimensions: [u32; 2] = surface.capabilities(device.physical_device())
                        .unwrap()
                        .min_image_extent;

                    let (new_swapchain, new_images) = match swapchain.recreate_with_dimensions(dimensions) {
                        Ok(r) => r,
                        Err(SwapchainCreationError::UnsupportedDimensions) => return,
                        Err(e) => panic!("Failed to recreate swapchain: {:?}", e)
                    };

                    swapchain = new_swapchain;
                    framebuffers = window_size_dependent_setup(
                        &new_images,
                        render_pass.clone(),
                        &mut dynamic_state,
                    );
                    recreate_swapchain = false;
                }

                let (image_num, suboptimal, acquire_future) = match swapchain::acquire_next_image(swapchain.clone(), None) {
                    Ok(r) => r,
                    Err(AcquireError::OutOfDate) => {
                        recreate_swapchain = true;
                        return;
                    }
                    Err(e) => panic!("Failed to acquire next image: {:?}", e)
                };

                recreate_swapchain = suboptimal;
                let clear_values = vec!([0.0, 0.0, 1.0, 1.0].into());

                let uniform_buffer_subbuffer = {
                    let elapsed = rotation_duration.elapsed();
                    let rotation = elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0;
                    let rotation = Matrix3::from_angle_z(Rad(rotation as f32));

                    let data = vs::ty::Data {
                        rotation: Matrix4::from(rotation).into()
                    };

                    uniform_buffer.next(data).unwrap()
                };

                let layout = pipeline.descriptor_set_layout(0).unwrap();
                let set = Arc::new(
                    PersistentDescriptorSet::start(
                        layout.clone()
                    )
                        .add_buffer(uniform_buffer_subbuffer)
                        .unwrap()
                        .build()
                        .unwrap()
                );

                let command_buffer = AutoCommandBufferBuilder::primary_one_time_submit(
                    device.clone(),
                    queue.family(),
                )
                    .unwrap()
                    .begin_render_pass(framebuffers[image_num].clone(), false, clear_values)
                    .unwrap()
                    .draw(
                        pipeline.clone(),
                        &dynamic_state,
                        vertex_buffer.clone(),
                        set.clone(),
                        (),
                    )
                    .unwrap()
                    .end_render_pass()
                    .unwrap()
                    .build()
                    .unwrap();

                let future = previous_frame_end.take()
                    .unwrap()
                    .join(acquire_future)
                    .then_execute(
                        queue.clone(),
                        command_buffer,
                    )
                    .unwrap()
                    .then_swapchain_present(
                        queue.clone(),
                        swapchain.clone(),
                        image_num,
                    )
                    .then_signal_fence_and_flush();

                match future {
                    Ok(future) => {
                        let _ = future.wait(None);
                        previous_frame_end = Some(Box::new(future) as Box<_>);
                    }
                    Err(FlushError::OutOfDate) => {
                        recreate_swapchain = true;
                        previous_frame_end = Some(Box::new(sync::now(device.clone())) as Box<_>);
                    }
                    Err(e) => {
                        println!("Failed to flush future: {:?}", e);
                        previous_frame_end = Some(Box::new(sync::now(device.clone())) as Box<_>);
                    }
                }
            }
            _ => ()
        }
    })
}

mod vs {
    vulkano_shaders::shader!{
        ty: "vertex",
        path: "src/vert.glsl"
    }
}

mod fs {
    vulkano_shaders::shader!{
        ty: "fragment",
        path: "src/frag.glsl"
    }
}