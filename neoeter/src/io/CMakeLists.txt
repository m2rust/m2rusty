cmake_minimum_required(VERSION 3.20)

# Standalone configuration when built directly
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    project(neoeter_io LANGUAGES C CXX VERSION 1.0.0)

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

    option(BUILD_TESTING "Build neoeter_io unit tests" ON)
    if(BUILD_TESTING)
        enable_testing()
    endif()
endif()

# Find dependencies: Threads and libzip
find_package(Threads REQUIRED)
find_package(PkgConfig QUIET)
if(PKG_CONFIG_FOUND)
    pkg_check_modules(LIBZIP QUIET libzip)
endif()
if(NOT LIBZIP_FOUND)
    find_path(LIBZIP_INCLUDE_DIRS NAMES zip.h)
    find_library(LIBZIP_LIBRARIES NAMES zip)
    if(NOT LIBZIP_INCLUDE_DIRS OR NOT LIBZIP_LIBRARIES)
        message(FATAL_ERROR "libzip is required to build neoeter_io (install libzip-dev / libzip).")
    endif()
endif()

# Source files
set(ETERPACK_SOURCES
    EterPackManager.cpp
    Folder.cpp
    ZIP.cpp
)

set(ETERPACK_HEADERS
    ${NEOETER_INCLUDE_DIR}/Neoeter/IO/EterPackManager.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/IO/FileProvider.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/IO/Folder.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/IO/Inline.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/IO/StdAfx.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/IO/ZIP.h
    ${NEOETER_INCLUDE_DIR}/Platform/Platform.h
)

add_library(neoeter_io STATIC
    ${ETERPACK_SOURCES}
    ${ETERPACK_HEADERS}
)
add_library(Neoeter::IO ALIAS neoeter_io)

target_include_directories(neoeter_io
    PUBLIC
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<INSTALL_INTERFACE:include>
        ${LIBZIP_INCLUDE_DIRS}
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
)

target_link_libraries(neoeter_io
    PUBLIC
        ${LIBZIP_LIBRARIES}
        Threads::Threads
    # libzip is static here and its zlib dependency is in Libs.private (T1.9).
    $<$<PLATFORM_ID:Windows>:z>
    $<$<PLATFORM_ID:Windows>:bcrypt>
)

# In monorepo, link with neoeter_platform_base
if(TARGET neoeter_platform_base)
    target_link_libraries(neoeter_io PUBLIC neoeter_platform_base)
endif()

# Unit Tests
if((CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR AND BUILD_TESTING) OR BUILD_TESTS)
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/tests/CMakeLists.txt")
        add_subdirectory(tests)
    endif()
endif()
