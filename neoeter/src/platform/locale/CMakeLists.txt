cmake_minimum_required(VERSION 3.20)

# Standalone configuration when built directly
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    project(neoeter_platform_locale LANGUAGES C CXX VERSION 1.0.0)

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

    option(BUILD_TESTING "Build neoeter_platform_locale unit tests" ON)
    if(BUILD_TESTING)
        enable_testing()
    endif()
endif()

# Source files
set(ETERLOCALE_SOURCES
    Arabic.cpp
    Japanese.cpp
    StdAfx.cpp
    StringCodec.cpp
    StringCodec_Vietnamese.cpp
)

set(ETERLOCALE_HEADERS
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Locale/Arabic.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Locale/CodePageId.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Locale/Japanese.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Locale/StdAfx.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Locale/StringCodec.h
    ${NEOETER_INCLUDE_DIR}/Neoeter/Platform/Locale/StringCodec_Vietnamese.h
    ${NEOETER_INCLUDE_DIR}/Platform/Platform.h
)

add_library(neoeter_platform_locale STATIC
    ${ETERLOCALE_SOURCES}
    ${ETERLOCALE_HEADERS}
)
add_library(Neoeter::Platform::Locale ALIAS neoeter_platform_locale)

target_include_directories(neoeter_platform_locale
    PUBLIC
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
)

# StringCodec uses iconv: part of glibc, a separate libiconv on macOS.
if(NOT WIN32)
    find_package(Iconv REQUIRED)
    target_link_libraries(neoeter_platform_locale PRIVATE Iconv::Iconv)
endif()

# In monorepo, link with neoeter_platform_base
if(TARGET neoeter_platform_base)
    target_link_libraries(neoeter_platform_locale PUBLIC neoeter_platform_base)
endif()

# Unit Tests
if((CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR AND BUILD_TESTING) OR BUILD_TESTS)
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/tests/CMakeLists.txt")
        add_subdirectory(tests)
    endif()
endif()
