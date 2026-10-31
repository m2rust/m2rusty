cmake_minimum_required(VERSION 3.16)
project(neoeter_effect LANGUAGES C CXX)

if(CMAKE_SOURCE_DIR STREQUAL PROJECT_SOURCE_DIR)
    set(EFFECTLIB_STANDALONE ON)
    set(CMAKE_CXX_STANDARD 17)
    set(CMAKE_CXX_STANDARD_REQUIRED ON)
    set(CMAKE_CXX_EXTENSIONS OFF)
    option(BUILD_TESTING "Build neoeter_effect tests" ON)
else()
    set(EFFECTLIB_STANDALONE OFF)
endif()

if(EFFECTLIB_STANDALONE)
    include(FetchContent)

    foreach(dep neoeter_platform_base:pkg_eterbase neoeter_platform_locale:pkg_eterlocale neoeter_io:pkg_eterpack neoeter_image:pkg_eterimagelib neoeter_platform_math:pkg_spherelib client_webbrowser:pkg_cwebbrowser neoeter_render:pkg_eterlib neoeter_audio:pkg_mileslib)
        string(REPLACE ":" ";" dep_list ${dep})
        list(GET dep_list 0 dep_name)
        list(GET dep_list 1 dep_repo)
        if(NOT TARGET ${dep_name})
            if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../${dep_name}/CMakeLists.txt")
                set(BUILD_TESTING_SAVED ${BUILD_TESTING})
                set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
                add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/../${dep_name}" "${CMAKE_BINARY_DIR}/deps/${dep_name}")
                set(BUILD_TESTING ${BUILD_TESTING_SAVED} CACHE BOOL "" FORCE)
            else()
                FetchContent_Declare(
                    ${dep_name}
                    GIT_REPOSITORY https://github.com/open-mt2/${dep_repo}.git
                    GIT_TAG main
                )
                set(BUILD_TESTING_SAVED ${BUILD_TESTING})
                set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
                FetchContent_MakeAvailable(${dep_name})
                set(BUILD_TESTING ${BUILD_TESTING_SAVED} CACHE BOOL "" FORCE)
            endif()
        endif()
    endforeach()
endif()

file(GLOB_RECURSE EFFECTLIB_SOURCES CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_SOURCE_DIR}/*.cpp"
)
list(FILTER EFFECTLIB_SOURCES EXCLUDE REGEX "/tests/")

add_library(neoeter_effect STATIC ${EFFECTLIB_SOURCES})
add_library(Neoeter::Effect ALIAS neoeter_effect)

target_include_directories(neoeter_effect
    PUBLIC
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
)

target_link_libraries(neoeter_effect
    PUBLIC
        neoeter_platform_base
        neoeter_io
        neoeter_render
        neoeter_audio
)

if(UNIX)
    target_compile_definitions(neoeter_effect PUBLIC
        OPENMT2_PLATFORM_LINUX=1
        OPENMT2_RENDERER_GL=1
        OPENMT2_SDL_WINDOW=1
        _GNU_SOURCE
    )
endif()

if((EFFECTLIB_STANDALONE AND BUILD_TESTING) OR (NOT EFFECTLIB_STANDALONE AND BUILD_TESTS))
    enable_testing()
    add_subdirectory(tests)
endif()
