cmake_minimum_required(VERSION 3.20)
project(Neoeter VERSION 0.1.0 LANGUAGES C CXX)

# Engine (L0-L3) of the OpenMT2 client. See docs/architecture.md.
#
# One directory per role; a role with more than one package has subpackages.

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")
include(NeoeterOptions)

set(NEOETER_INCLUDE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/include")
include_directories("${NEOETER_INCLUDE_DIR}")

set(NEOETER_PACKAGES
    platform/base
    platform/locale
    platform/math
    io
    image
    render
    audio
    model/grn
    model/speedtree
    terrain
    effect
)

foreach(_package IN LISTS NEOETER_PACKAGES)
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/src/${_package}/CMakeLists.txt")
        # One binary directory per role/subpackage; keeps the build tree free of
        # the historic package names.
        add_subdirectory("src/${_package}" "${CMAKE_BINARY_DIR}/src/${_package}")
    else()
        message(STATUS "Neoeter: src/${_package} not imported yet")
    endif()
endforeach()

include(NeoeterAliases)

if(BUILD_TESTS AND EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/tests/CMakeLists.txt")
    enable_testing()
    add_subdirectory(tests)
endif()

message(STATUS "Neoeter configured (${NEOETER_PACKAGES})")
