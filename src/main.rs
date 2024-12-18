#![allow(unused)]

use anyhow::{Context, Result};
use ash::{
    self,
    vk::{
        self, make_api_version, ApplicationInfo, Buffer, BufferCreateInfo, CommandBuffer,
        CommandBufferAllocateInfo, CommandBufferBeginInfo, CommandBufferUsageFlags, CommandPool,
        CommandPoolCreateInfo, DebugUtilsMessengerCreateInfoEXT, DeviceCreateInfo,
        DeviceQueueCreateInfo, Fence, FenceCreateFlags, FenceCreateInfo, InstanceCreateInfo,
        MemoryRequirements, PhysicalDevice, Queue, SubmitInfo,
    },
    Device, Entry, Instance,
};
use gpu_allocator::vulkan::*;
use gpu_allocator::MemoryLocation;
use rand::Rng;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::os::raw::c_void;
use std::ptr;
use std::time;
use std::{time::Instant, u64};

const VALIDATION_ENABLED: bool = cfg!(debug_assertions);

pub fn vk_to_string(raw_string_array: &[c_char]) -> String {
    let raw_string = unsafe {
        let pointer = raw_string_array.as_ptr();
        CStr::from_ptr(pointer)
    };

    raw_string
        .to_str()
        .expect("Failed to convert vulkan raw string.")
        .to_owned()
}

unsafe extern "system" fn vulkan_debug_utils_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    let severity = match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => "[Verbose]",
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => "[Warning]",
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => "[Error]",
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => "[Info]",
        _ => "[Unknown]",
    };
    let types = match message_type {
        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "[General]",
        vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "[Performance]",
        vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "[Validation]",
        _ => "[Unknown]",
    };
    let message = CStr::from_ptr((*p_callback_data).p_message);
    println!("[Debug]{}{}{:?}", severity, types, message);

    vk::FALSE
}

fn main() -> Result<()> {
    // Data
    let width: u64 = 1280;
    let height: u64 = 720;
    let value_count: u64 = width * height;
    let red: u32 = rand::thread_rng().gen_range(0..255);
    let green: u32 = rand::thread_rng().gen_range(0..255);
    let blue: u32 = rand::thread_rng().gen_range(0..255);
    let alpha: u32 = 255;
    let value: u32 = red | green << 8 | blue << 16 | alpha << 24;

    // Ash setup
    let entry: Entry = unsafe { ash::Entry::load() }?;

    // Enable validation layer

    // Setup Instance
    let instance: Instance = {
        let application_name = CString::new(env!("CARGO_PKG_NAME")).unwrap();
        let application_version: u32 = vk::make_api_version(
            0,
            env!("CARGO_PKG_VERSION_MAJOR").parse::<u32>().unwrap(),
            env!("CARGO_PKG_VERSION_MINOR").parse::<u32>().unwrap(),
            env!("CARGO_PKG_VERSION_PATCH").parse::<u32>().unwrap(),
        );
        let application_info: ApplicationInfo = vk::ApplicationInfo::default()
            .api_version(vk::API_VERSION_1_3)
            .application_name(application_name.as_c_str())
            .application_version(application_version);

        let mut create_info: InstanceCreateInfo =
            vk::InstanceCreateInfo::default().application_info(&application_info);

        // Set up the validation layer
        if (VALIDATION_ENABLED) {
            let validation_layer_name: CString =
                CString::new("VK_LAYER_KHRONOS_validation").unwrap();

            unsafe {
                let layer_properties = entry.enumerate_instance_layer_properties()?;

                if (layer_properties.len() <= 0) {
                    println!("[VK] No validation layer");
                }

                let mut validation_layer_found = false;
                for layer_property in layer_properties.iter() {
                    let test_layer_name: String = vk_to_string(&layer_property.layer_name);
                    if (test_layer_name == "VK_LAYER_KHRONOS_validation") {
                        println!("[VK] Validation Layer found");
                        validation_layer_found = true;
                        break;
                    }
                }
            }

            let debug_create_info: DebugUtilsMessengerCreateInfoEXT =
                vk::DebugUtilsMessengerCreateInfoEXT::default()
                    .message_severity(
                        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                            | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
                    )
                    .message_type(
                        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                            | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
                    )
                    .pfn_user_callback(Some(vulkan_debug_utils_callback));

            let validation_layer_names: Vec<*const i8> = vec![validation_layer_name.as_ptr()];

            create_info.p_next =
                &debug_create_info as *const vk::DebugUtilsMessengerCreateInfoEXT as *const c_void;
            create_info.enabled_layer_names(&validation_layer_names);
        }
        unsafe { entry.create_instance(&create_info, None) }?
    };

    // Build Device
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

    // Setup Queue
    let queue: Queue = unsafe { device.get_device_queue(0, 0) };

    // Set up Buffer
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

    // Setup CommandPool
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
