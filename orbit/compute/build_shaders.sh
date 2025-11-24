#!/bin/bash
# Build script to compile Vulkan GLSL shaders to SPIR-V
# Requires: glslc (shaderc package)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SHADER_DIR="${SCRIPT_DIR}/src/shaders/vulkan"
OUTPUT_DIR="${SHADER_DIR}"

echo "Compiling Vulkan shaders..."

# Check for glslc in custom location first, then system PATH
if [ -f "$HOME/gsl/bin/glslc" ]; then
    GLSC_PATH="$HOME/gsl/bin/glslc"
    echo "Using glslc from: $GLSC_PATH"
elif command -v glslc &> /dev/null; then
    GLSC_PATH="glslc"
    echo "Using glslc from system PATH"
else
    echo "Error: glslc not found. Please install shaderc:"
    echo "  macOS: brew install shaderc"
    echo "  Linux: sudo apt-get install shaderc"
    echo "  Or download from: https://github.com/google/shaderc"
    echo "  Or set GLSC_PATH environment variable to point to glslc"
    exit 1
fi

cd "${SHADER_DIR}"

# Compile shaders
echo "Compiling graph_bfs.comp..."
"$GLSC_PATH" -fshader-stage=compute graph_bfs.comp -o graph_bfs.spv || {
    echo "Warning: Failed to compile graph_bfs.comp"
}

echo "Compiling vector_similarity.comp..."
"$GLSC_PATH" -fshader-stage=compute vector_similarity.comp -o vector_similarity.spv || {
    echo "Warning: Failed to compile vector_similarity.comp"
}

echo "Compiling graph_connected_components.comp..."
"$GLSC_PATH" -fshader-stage=compute graph_connected_components.comp -o graph_connected_components.spv || {
    echo "Warning: Failed to compile graph_connected_components.comp"
}

echo "✅ Shader compilation complete!"
echo "Compiled SPIR-V files are in: ${OUTPUT_DIR}"

