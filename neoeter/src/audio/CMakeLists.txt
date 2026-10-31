cmake_minimum_required(VERSION 3.20)

# Standalone configuration when built directly
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    project(neoeter_audio LANGUAGES C CXX VERSION 1.0.0)

    set(CMAKE_CXX_STANDARD 17)
    set(CMAKE_CXX_STANDARD_REQUIRED ON)
    set(CMAKE_CXX_EXTENSIONS OFF)

    if(WIN32)
        add_compile_definitions(WIN32 _WINDOWS _CRT_SECURE_NO_WARNINGS)
    else()
        add_compile_definitions(
            OPENMT2_PLATFORM_LINUX=1
            OPENMT2_RENDERER_GL=1
            OPENMT2_SDL_WINDOW=1
            _GNU_SOURCE
        )
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

    option(BUILD_TESTING "Build neoeter_audio unit tests" ON)
    if(BUILD_TESTING)
        enable_testing()
    endif()

    # Resolve sibling dependencies when built standalone inside the workspace or with deps/
    set(_MILESLIB_DEPS neoeter_platform_base neoeter_platform_locale neoeter_io neoeter_image neoeter_platform_math neoeter_render)
    foreach(_dep IN LISTS _MILESLIB_DEPS)
        if(NOT TARGET ${_dep})
            if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../${_dep}/CMakeLists.txt")
                add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/../${_dep}" "${CMAKE_BINARY_DIR}/deps/${_dep}" EXCLUDE_FROM_ALL)
            elseif(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/deps/${_dep}/CMakeLists.txt")
                add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/deps/${_dep}" "${CMAKE_BINARY_DIR}/deps/${_dep}" EXCLUDE_FROM_ALL)
            endif()
        endif()
    endforeach()
endif()

find_package(Threads REQUIRED)
find_package(PkgConfig REQUIRED)
pkg_check_modules(MINIAUDIO REQUIRED miniaudio)

set(MILESLIB_SOURCES
    SoundBase.cpp
    SoundData.cpp
    SoundInstance2D.cpp
    SoundInstance3D.cpp
    SoundInstanceStream.cpp
    SoundManager.cpp
    SoundManager2D.cpp
    SoundManager3D.cpp
    SoundManagerStream.cpp
    StdAfx.cpp
    Type.cpp
)

set(MILESLIB_HEADERS
    ${NEOETER_INCLUDE_DIR}/Neoeter/Audio/MSSCompat.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Audio/SoundBase.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Audio/SoundData.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Audio/SoundInstance.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Audio/SoundManager.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Audio/SoundManager2D.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Audio/SoundManager3D.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Audio/SoundManagerStream.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Audio/StdAfx.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Audio/Type.h
    ${NEOETER_INCLUDE_DIR}/Platform/DirectXMathCompat.h
    ${NEOETER_INCLUDE_DIR}/Platform/Platform.h
)

add_library(neoeter_audio STATIC
    ${MILESLIB_SOURCES}
    ${MILESLIB_HEADERS}
)
add_library(Neoeter::Audio ALIAS neoeter_audio)

target_include_directories(neoeter_audio
    PUBLIC
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<INSTALL_INTERFACE:include>
        ${MINIAUDIO_INCLUDE_DIRS}
        /usr/include/miniaudio
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
)

target_link_libraries(neoeter_audio
    PUBLIC
        neoeter_platform_base
        neoeter_io
        neoeter_render
        Threads::Threads
        ${MINIAUDIO_LIBRARIES}
        $<$<PLATFORM_ID:Linux>:dl>
        $<$<PLATFORM_ID:Linux>:m>
)

# Unit Tests
if((CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR AND BUILD_TESTING) OR BUILD_TESTS)
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/tests/CMakeLists.txt")
        add_subdirectory(tests)
    endif()
endif()
