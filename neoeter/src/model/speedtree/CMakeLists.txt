cmake_minimum_required(VERSION 3.16)
project(neoeter_model_speedtree LANGUAGES C CXX)

if(CMAKE_SOURCE_DIR STREQUAL PROJECT_SOURCE_DIR)
    set(SPEEDTREELIB_STANDALONE ON)
    set(CMAKE_CXX_STANDARD 17)
    set(CMAKE_CXX_STANDARD_REQUIRED ON)
    set(CMAKE_CXX_EXTENSIONS OFF)
    option(BUILD_TESTING "Build neoeter_model_speedtree tests" ON)
else()
    set(SPEEDTREELIB_STANDALONE OFF)
endif()

if(SPEEDTREELIB_STANDALONE)
    include(FetchContent)

    foreach(dep neoeter_platform_base:pkg_eterbase neoeter_platform_locale:pkg_eterlocale neoeter_io:pkg_eterpack neoeter_image:pkg_eterimagelib neoeter_platform_math:pkg_spherelib neoeter_render:pkg_eterlib)
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

file(GLOB_RECURSE SPEEDTREELIB_SOURCES CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_SOURCE_DIR}/*.cpp"
)
list(FILTER SPEEDTREELIB_SOURCES EXCLUDE REGEX "/tests/")

add_library(neoeter_model_speedtree STATIC ${SPEEDTREELIB_SOURCES})
add_library(Neoeter::Model::SpeedTree ALIAS neoeter_model_speedtree)

target_include_directories(neoeter_model_speedtree
    PUBLIC
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<BUILD_INTERFACE:${NEOETER_INCLUDE_DIR}>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
)

target_link_libraries(neoeter_model_speedtree
    PUBLIC
        neoeter_platform_base
        neoeter_io
        neoeter_render
)

if(UNIX)
    target_compile_definitions(neoeter_model_speedtree PUBLIC
        OPENMT2_PLATFORM_LINUX=1
        OPENMT2_RENDERER_GL=1
        OPENMT2_SDL_WINDOW=1
        _GNU_SOURCE
    )
endif()

if((SPEEDTREELIB_STANDALONE AND BUILD_TESTING) OR (NOT SPEEDTREELIB_STANDALONE AND BUILD_TESTS))
    enable_testing()
    add_subdirectory(tests)
endif()
