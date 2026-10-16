cmake_minimum_required(VERSION 3.10)

project(speexdsp LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 14)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

option(BUILD_SHARED_LIBS "Build shared libraries" OFF)
option(USE_FIXED_POINT "Use fixed-point arithmetic" ON)

# Define compile-time options
if(USE_FIXED_POINT)
    add_compile_definitions(FIXED_POINT=1)
else()
    add_compile_definitions(FLOATING_POINT=1)
endif()

# 'M_PI': undeclared identifier
if(WIN32)
    add_compile_definitions(_USE_MATH_DEFINES=1)
endif()


add_compile_definitions(EXPORT= USE_KISS_FFT=1)
# Sherpa ONNX includes a floating-point KISS FFT with the same public symbols.
# Keep SpeexDSP's fixed-point ABI private to this archive.
add_compile_definitions(
  kiss_fft_alloc=spx_kiss_fft_alloc
  kiss_fft_stride=spx_kiss_fft_stride
  kiss_fft=spx_kiss_fft
  kiss_fftr_alloc=spx_kiss_fftr_alloc
  kiss_fftr=spx_kiss_fftr
  kiss_fftri=spx_kiss_fftri
  kiss_fftr2=spx_kiss_fftr2
  kiss_fftri2=spx_kiss_fftri2
)

# Include directories
include_directories(
    ${CMAKE_SOURCE_DIR}/include
    ${CMAKE_SOURCE_DIR}/libspeexdsp
)

# Source files
file(GLOB SPEEXDSP_SOURCES libspeexdsp/*.c)
add_library(speexdsp ${SPEEXDSP_SOURCES})

# Installation rules
install(TARGETS speexdsp
    EXPORT speexdspConfig
    RUNTIME DESTINATION bin
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
)
install(DIRECTORY include/ DESTINATION include)
