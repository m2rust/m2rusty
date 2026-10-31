cmake_minimum_required(VERSION 3.20)

# Standalone configuration when built directly
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    project(neoeter_render LANGUAGES C CXX VERSION 1.0.0)

    set(CMAKE_CXX_STANDARD 17)
    set(CMAKE_CXX_STANDARD_REQUIRED ON)
    set(CMAKE_CXX_EXTENSIONS OFF)
    set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

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

    option(BUILD_TESTING "Build neoeter_render unit tests" ON)
    if(BUILD_TESTING)
        enable_testing()
    endif()

    # Resolve sibling dependencies when built standalone inside the workspace or with deps/
    set(_ETERLIB_DEPS neoeter_platform_base neoeter_platform_locale neoeter_io neoeter_image neoeter_platform_math)
    foreach(_dep IN LISTS _ETERLIB_DEPS)
        if(NOT TARGET ${_dep})
            if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../${_dep}/CMakeLists.txt")
                add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/../${_dep}" "${CMAKE_BINARY_DIR}/deps/${_dep}" EXCLUDE_FROM_ALL)
            elseif(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/deps/${_dep}/CMakeLists.txt")
                add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/deps/${_dep}" "${CMAKE_BINARY_DIR}/deps/${_dep}" EXCLUDE_FROM_ALL)
            endif()
        endif()
    endforeach()
endif()

# Find dependencies: Threads, SDL2 (both targets since T1.2), OpenGL/Freetype
find_package(Threads REQUIRED)
find_package(SDL2 REQUIRED)
if(UNIX)
    find_package(OpenGL REQUIRED)
    find_package(Freetype REQUIRED)
endif()

file(GLOB ETERLIB_SOURCES CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/*.cpp")
file(GLOB ETERLIB_HEADERS CONFIGURE_DEPENDS
    "${NEOETER_INCLUDE_DIR}/Neoeter/Render/*.h"
    "${NEOETER_INCLUDE_DIR}/Platform/*.h"
)

add_library(neoeter_render STATIC
    ${ETERLIB_SOURCES}
    ${ETERLIB_HEADERS}
)
add_library(Neoeter::Render ALIAS neoeter_render)

target_include_directories(neoeter_render
    PUBLIC
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
)

target_link_libraries(neoeter_render
    PUBLIC
        neoeter_platform_base
        neoeter_platform_locale
        neoeter_io
        neoeter_image
        neoeter_platform_math
        Threads::Threads
        SDL2::SDL2
        $<$<PLATFORM_ID:Linux,Darwin>:OpenGL::GL>
        $<$<PLATFORM_ID:Linux,Darwin>:Freetype::Freetype>
)

# Optional: resolve font face names to files (FontLocator.cpp). Without it the
# lookup falls back to fixed FHS paths, which don't exist on NixOS.
if(UNIX)
    find_package(Fontconfig QUIET)
    if(Fontconfig_FOUND)
        target_link_libraries(neoeter_render PRIVATE Fontconfig::Fontconfig)
        target_compile_definitions(neoeter_render PRIVATE OPENMT2_HAVE_FONTCONFIG=1)
    else()
        message(STATUS "neoeter_render: fontconfig not found, using fixed font paths")
    endif()
endif()

# Unit Tests
if((CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR AND BUILD_TESTING) OR BUILD_TESTS)
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/tests/CMakeLists.txt")
        add_subdirectory(tests)
    endif()
endif()
