cmake_minimum_required(VERSION 3.16)
project(neoeter_python_host LANGUAGES C CXX)

if(CMAKE_SOURCE_DIR STREQUAL PROJECT_SOURCE_DIR)
    set(SCRIPTLIB_STANDALONE ON)
    set(CMAKE_CXX_STANDARD 17)
    set(CMAKE_CXX_STANDARD_REQUIRED ON)
    set(CMAKE_CXX_EXTENSIONS OFF)
    option(BUILD_TESTING "Build neoeter_python_host tests" ON)
else()
    set(SCRIPTLIB_STANDALONE OFF)
endif()

if(SCRIPTLIB_STANDALONE)
    include(FetchContent)

    foreach(dep EterBase:pkg_eterbase EterLocale:pkg_eterlocale EterPack:pkg_eterpack EterImageLib:pkg_eterimagelib SphereLib:pkg_spherelib client_webbrowser:pkg_cwebbrowser EterLib:pkg_eterlib MilesLib:pkg_mileslib EterGrnLib:pkg_etergrnlib EffectLib:pkg_effectlib)
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

# Locate Python 2.7
find_package(PythonLibs 2.7 QUIET)

if(NOT PYTHON_LIBRARIES)
    find_library(PYTHON_LIBRARIES NAMES python2.7 python27 libpython2.7 libpython27
        HINTS
            ${CMAKE_CURRENT_SOURCE_DIR}/../../extern/lib
            ${CMAKE_CURRENT_SOURCE_DIR}/../extern/lib
            ${CMAKE_CURRENT_SOURCE_DIR}/extern/lib
            ${CMAKE_SOURCE_DIR}/extern/lib
            /usr/lib
            /usr/lib/x86_64-linux-gnu
            /usr/local/lib
    )
endif()

if(NOT PYTHON_INCLUDE_DIRS)
    find_path(PYTHON_INCLUDE_DIRS NAMES python2.7/Python.h Python.h
        HINTS
            ${NEOETER_PYTHON_INCLUDE_DIR}/python2.7
            ${NEOETER_PYTHON_INCLUDE_DIR}
            ${CMAKE_CURRENT_SOURCE_DIR}/../../extern/include/python2.7
            ${CMAKE_CURRENT_SOURCE_DIR}/../../extern/include
            ${CMAKE_SOURCE_DIR}/extern/include/python2.7
            ${CMAKE_SOURCE_DIR}/extern/include
            /usr/include/python2.7
            /usr/include
    )
endif()

file(GLOB_RECURSE SCRIPTLIB_SOURCES CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_SOURCE_DIR}/*.cpp"
)
list(FILTER SCRIPTLIB_SOURCES EXCLUDE REGEX "/tests/")

add_library(neoeter_python_host STATIC ${SCRIPTLIB_SOURCES})
add_library(NeoeterPython::Host ALIAS neoeter_python_host)

target_include_directories(neoeter_python_host
    PUBLIC
        $<BUILD_INTERFACE:${NEOETER_PYTHON_INCLUDE_DIR}>
        $<BUILD_INTERFACE:${NEOETER_PYTHON_INCLUDE_DIR}>
        # The vendored copy is a Linux Python 2.7 include set; on Windows the
        # headers must come from the cross-built prefix (T2.4/T1.9).
        $<$<NOT:$<PLATFORM_ID:Windows>>:$<BUILD_INTERFACE:${NEOETER_PYTHON_INCLUDE_DIR}/python2.7>>
        ${PYTHON_INCLUDE_DIRS}
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
)

target_link_libraries(neoeter_python_host
    PUBLIC
        neoeter_platform_base
        neoeter_io
        neoeter_render
        neoeter_model_grn
        neoeter_effect
        ${PYTHON_LIBRARIES}
        $<$<PLATFORM_ID:Linux>:util>
        $<$<PLATFORM_ID:Linux>:dl>
)

# The launcher overrides PYTHONHOME, so record where the linked libpython's
# stdlib lives as a fallback search location (see PythonLauncher.cpp).
if(UNIX AND PYTHON_LIBRARIES)
    foreach(_pylib IN LISTS PYTHON_LIBRARIES)
        if(_pylib MATCHES "python2\\.?7")
            get_filename_component(_pylib_real "${_pylib}" REALPATH)
            get_filename_component(_pylib_dir "${_pylib_real}" DIRECTORY)
            set(NEOETER_PYTHON_STDLIB "${_pylib_dir}/python2.7")
            break()
        endif()
    endforeach()
    if(NEOETER_PYTHON_STDLIB)
        message(STATUS "neoeter_python_host: build-time Python stdlib ${NEOETER_PYTHON_STDLIB}")
        target_compile_definitions(neoeter_python_host PRIVATE
            OPENMT2_PYTHON_STDLIB_BUILD="${NEOETER_PYTHON_STDLIB}")
    endif()
endif()

if(UNIX)
    target_compile_definitions(neoeter_python_host PUBLIC
        OPENMT2_PLATFORM_LINUX=1
        OPENMT2_RENDERER_GL=1
        OPENMT2_SDL_WINDOW=1
        _GNU_SOURCE
    )
    target_compile_options(neoeter_python_host PUBLIC -Wno-register)
    if(CMAKE_CXX_COMPILER_ID MATCHES "Clang")
        target_compile_options(neoeter_python_host PUBLIC -Wno-macro-redefined)
    endif()
endif()

if((SCRIPTLIB_STANDALONE AND BUILD_TESTING) OR (NOT SCRIPTLIB_STANDALONE AND BUILD_TESTS))
    enable_testing()
    add_subdirectory(tests)
endif()
