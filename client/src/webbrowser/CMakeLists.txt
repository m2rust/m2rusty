cmake_minimum_required(VERSION 3.20)

# Standalone configuration when built directly
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    project(client_webbrowser LANGUAGES C CXX VERSION 1.0.0)

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

    option(BUILD_TESTING "Build client_webbrowser unit tests" ON)
    if(BUILD_TESTING)
        enable_testing()
    endif()
endif()

set(CWEBBROWSER_SOURCES
    CWebBrowser.cpp
)

set(CWEBBROWSER_HEADERS
    ${CLIENT_INCLUDE_DIR}/Client/WebBrowser/CWebBrowser.h
)

add_library(client_webbrowser STATIC
    ${CWEBBROWSER_SOURCES}
    ${CWEBBROWSER_HEADERS}
)
add_library(Client::WebBrowser ALIAS client_webbrowser)

target_include_directories(client_webbrowser
    PUBLIC
        $<BUILD_INTERFACE:${CLIENT_INCLUDE_DIR}>
        $<BUILD_INTERFACE:${CLIENT_INCLUDE_DIR}>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
)

# Unit Tests
if((CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR AND BUILD_TESTING) OR BUILD_TESTS)
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/tests/CMakeLists.txt")
        add_subdirectory(tests)
    endif()
endif()

if(TARGET neoeter_platform_base)
    target_link_libraries(client_webbrowser PUBLIC neoeter_platform_base)
endif()
