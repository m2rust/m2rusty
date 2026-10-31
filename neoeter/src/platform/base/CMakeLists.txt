cmake_minimum_required(VERSION 3.20)

# Standalone configuration when built directly
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    project(neoeter_platform_base LANGUAGES C CXX VERSION 1.0.0)

    set(CMAKE_CXX_STANDARD 17)
    set(CMAKE_CXX_STANDARD_REQUIRED ON)
    set(CMAKE_CXX_EXTENSIONS OFF)

    if(NOT WIN32)
        add_compile_options(
            -Wall
            -Wextra
            -Wno-unused-parameter
            -Wno-invalid-offsetof
            -Wno-multichar
            -Wno-sign-compare
            -fPIC
        )
    endif()

    option(BUILD_TESTING "Build neoeter_platform_base unit tests" ON)
    if(BUILD_TESTING)
        enable_testing()
    endif()
endif()

# Dependencies
find_package(PkgConfig QUIET)
find_library(LZO_LIBRARY NAMES lzo2 lzo REQUIRED)
# lzo.h is a public neoeter_platform_base header, so its include directory has to be public
# too; on Windows cross builds it comes from the deps prefix (T2.3), not the host.
find_path(LZO_INCLUDE_DIR NAMES lzo/lzo1x.h)
if(NOT WIN32)
    # Windows cross builds link opengl32 from the MinGW sysroot instead (T1.5).
    find_package(OpenGL REQUIRED)
endif()

# Source files
set(ETERBASE_SOURCES
    CPostIt.cpp
    CRC32.cpp
    Debug.cpp
    Direct3DDevice9Compat.cpp
    error.cpp
    FileBase.cpp
    FileDir.cpp
    FileLoader.cpp
    ImageLoaderCompat.cpp
    ImageWriterCompat.cpp
    lzo.cpp
    MappedFile.cpp
    Random.cpp
    RenderBackend.cpp
    StdAfx.cpp
    Stl.cpp
    tea.cpp
    TempFile.cpp
    Timer.cpp
    Utils.cpp
    backends/gl/GLCompat.cpp
    backends/gl/GLRenderBackend.cpp
    Poly/Base.cpp
    Poly/Poly.cpp
    Poly/Symbol.cpp
    Poly/SymTable.cpp
)

set(ETERBASE_HEADERS
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/CPostIt.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/CRC32.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/Debug.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/error.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/FileBase.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/FileDir.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/FileLoader.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/Filename.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/ImageLoaderCompat.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/lzo.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/MappedFile.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/Random.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/ServiceDefs.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/Singleton.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/StdAfx.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/Stl.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/tea.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/TempFile.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/Timer.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/Utils.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/vk.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/Poly/Base.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/Poly/Poly.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/Poly/Symbol.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Base/Poly/SymTable.h
    ${NEOETER_INCLUDE_DIR}/Platform/Platform.h
    ${NEOETER_INCLUDE_DIR}/Platform/GLCompat.h
    ${NEOETER_INCLUDE_DIR}/Platform/RenderBackend.h
    ${NEOETER_INCLUDE_DIR}/Platform/stb_image.h
    ${NEOETER_INCLUDE_DIR}/Platform/stb_image_write.h
)

add_library(neoeter_platform_base STATIC
    ${ETERBASE_SOURCES}
    ${ETERBASE_HEADERS}
)
add_library(Neoeter::Platform::Base ALIAS neoeter_platform_base)

# Platform macros must be PUBLIC: consumers (standalone neoeter_render, neoeter_python_host,
# neoeter_model_grn, neoeter_model_speedtree, neoeter_terrain, neoeter_python_bindings) include the Platform
# headers and only see definitions carried by the target they link (issue #79).
if(WIN32)
    # T1.5: the Windows build uses the same GL compatibility renderer as Linux.
    # USE_PLATFORM_D3D_COMPAT keeps DirectXMathCompat.h as the single D3D9
    # vocabulary instead of pulling d3d9/d3dx9/dinput from the SDK.
    target_compile_definitions(neoeter_platform_base PUBLIC
        WIN32
        _WINDOWS
        _CRT_SECURE_NO_WARNINGS
        OPENMT2_PLATFORM_WINDOWS=1
        OPENMT2_RENDERER_GL=1
        OPENMT2_SDL_WINDOW=1
        USE_PLATFORM_D3D_COMPAT=1
        MS_WIN64
        STRSAFE_NO_DEPRECATE
    )
else()
    target_compile_definitions(neoeter_platform_base PUBLIC
        OPENMT2_PLATFORM_LINUX=1
        OPENMT2_RENDERER_GL=1
        OPENMT2_SDL_WINDOW=1
        _GNU_SOURCE
    )
endif()

target_include_directories(neoeter_platform_base
    PUBLIC
        ${LZO_INCLUDE_DIR}
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
        ${CMAKE_CURRENT_SOURCE_DIR}/Poly
)

target_link_libraries(neoeter_platform_base
    PUBLIC
        ${LZO_LIBRARY}
        $<$<PLATFORM_ID:Linux,Darwin>:OpenGL::GL>
        # Windows has only GL 1.1 in opengl32.dll; the entry points above it are
        # loaded at runtime (Platform_LoadGLFunctions, T1.4).
        $<$<PLATFORM_ID:Windows>:opengl32>
        # Timer.cpp uses timeGetTime (T1.5: the first Windows link blocker
        # after the POSIX libraries were made platform-aware in T2.5).
        $<$<PLATFORM_ID:Windows>:winmm>
        # error.cpp writes the crash log through imagehlp and winsock (T4.1
        # removed the MSVC #pragma comment lines that used to carry these).
        $<$<PLATFORM_ID:Windows>:imagehlp>
        $<$<PLATFORM_ID:Windows>:ws2_32>
)

# Unit Tests
if((CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR AND BUILD_TESTING) OR BUILD_TESTS)
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/tests/CMakeLists.txt")
        add_subdirectory(tests)
    endif()
endif()
