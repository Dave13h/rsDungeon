#![allow(unused)]

use std::{time::Instant, u64};

use anyhow::{Context, Result};
use ash::{
    self,
    vk::{
        self, ApplicationInfo, Buffer, BufferCreateInfo, CommandBuffer, CommandBufferAllocateInfo,
        CommandBufferBeginInfo, CommandBufferUsageFlags, CommandPool, CommandPoolCreateInfo,
        DeviceCreateInfo, DeviceQueueCreateInfo, Fence, FenceCreateFlags, FenceCreateInfo,
        InstanceCreateInfo, MemoryRequirements, PhysicalDevice, Queue, SubmitInfo,
    },
    Device, Entry, Instance,
};
use gpu_allocator::vulkan::*;
use gpu_allocator::MemoryLocation;
use std::time;

fn main() -> Result<()> {
    // Data
    let width: u64 = 1280;
    let height: u64 = 720;
    let value_count: u64 = width * height;
    let red: u32 = 255;
    let green: u32 = 0;
    let blue: u32 = 255;
    let alpha: u32 = 255;
    let value: u32 = red | green << 8 | blue << 16 | alpha << 24;

    // Ash setup
    let entry: Entry = unsafe { ash::Entry::load() }?;

    let instance: Instance = {
        let application_info: ApplicationInfo =
            vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_3);

        let create_info: InstanceCreateInfo =
            vk::InstanceCreateInfo::default().application_info(&application_info);
        unsafe { entry.create_instance(&create_info, None) }?
    };

    let physical_device: PhysicalDevice = unsafe { instance.enumerate_physical_devices() }?
        .into_iter()
        .next()
        .context("No physical Device Found")?;

    let device: Device = {
        let queue_priorities: [f32; 1] = [1.0];
        let queue_create_infos: [DeviceQueueCreateInfo; 1] = [DeviceQueueCreateInfo::default()
            .queue_family_index(0)
            .queue_priorities(&queue_priorities)];

        let create_info: DeviceCreateInfo =
            vk::DeviceCreateInfo::default().queue_create_infos(&queue_create_infos);

        unsafe { instance.create_device(physical_device, &create_info, None) }?
    };

    let queue: Queue = unsafe { device.get_device_queue(0, 0) };

    let mut allocator = Allocator::new(&AllocatorCreateDesc {
        instance: instance.clone(),
        device: device.clone(),
        physical_device,
        debug_settings: Default::default(),
        buffer_device_address: true,
        allocation_sizes: Default::default(),
    })?;

    let buffer: Buffer = {
        let create_info: BufferCreateInfo = vk::BufferCreateInfo::default()
            .size(value_count * std::mem::size_of::<u32>() as vk::DeviceSize)
            .usage(vk::BufferUsageFlags::TRANSFER_DST);

        unsafe { device.create_buffer(&create_info, None) }?
    };

    // Create Allocator
    let allocation: Allocation = {
        let memory_requirements: vk::MemoryRequirements =
            unsafe { device.get_buffer_memory_requirements(buffer) };

        let allocation_create_description = AllocationCreateDesc {
            name: "Example allocation",
            requirements: memory_requirements,
            location: MemoryLocation::GpuToCpu,
            linear: true, // Buffers are always linear
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        };

        let allocation: Allocation = allocator.allocate(&allocation_create_description)?;

        unsafe { device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset()) };

        allocation
    };

    let command_pool: CommandPool = {
        let create_info: CommandPoolCreateInfo =
            vk::CommandPoolCreateInfo::default().queue_family_index(0);

        unsafe { device.create_command_pool(&create_info, None) }?
    };

    // Create Buffers
    let command_buffer: CommandBuffer = {
        let create_info: CommandBufferAllocateInfo = vk::CommandBufferAllocateInfo::default()
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_pool(command_pool)
            .command_buffer_count(1);

        unsafe {
            device
                .allocate_command_buffers(&create_info)?
                .into_iter()
                .next()
                .context("No Command Buffers")
        }?
    };

    // Recording Command Buffer
    {
        let begin_info: CommandBufferBeginInfo =
            vk::CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe { device.begin_command_buffer(command_buffer, &begin_info) }?;
    }

    unsafe {
        device.cmd_fill_buffer(
            command_buffer,
            buffer,
            allocation.offset(),
            allocation.size(),
            value,
        );
    }

    unsafe { device.end_command_buffer(command_buffer) }?;

    // Execute Command Buffer
    let fence: Fence = {
        let create_info: FenceCreateInfo = vk::FenceCreateInfo::default();
        unsafe { device.create_fence(&create_info, None) }?
    };

    {
        let submit_info: SubmitInfo =
            vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&command_buffer));
        unsafe { device.queue_submit(queue, std::slice::from_ref(&submit_info), fence) };
    }

    // Wait for execution
    let gpu_start: Instant = std::time::Instant::now();
    unsafe { device.wait_for_fences(std::slice::from_ref(&fence), true, u64::MAX) }?;
    println!("GPU took {:?}", std::time::Instant::now() - gpu_start);

    // Read back
    let data: &[u8] = allocation
        .mapped_slice()
        .context("Host cannot access buffer")?;

    let png_start: Instant = std::time::Instant::now();
    image::save_buffer(
        "tmp/image.png",
        data,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
    );
    println!("PNG took {:?}", std::time::Instant::now() - png_start);

    // Cleanup
    unsafe { device.destroy_fence(fence, None) };
    unsafe { device.destroy_command_pool(command_pool, None) }
    allocator.free(allocation)?;
    drop(allocator);
    unsafe { device.destroy_buffer(buffer, None) };
    unsafe { device.destroy_device(None) }
    unsafe { instance.destroy_instance(None) }
    Ok(())
}
