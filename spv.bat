@echo off
echo Compiling Basic Shader...
"C:\Program Files\VulkanSDK\1.3.296.0\Bin\glslc.exe" .\shaders\basic.vert -o .\shaders\basic.vert.spv
"C:\Program Files\VulkanSDK\1.3.296.0\Bin\glslc.exe" .\shaders\basic.frag -o .\shaders\basic.frag.spv
