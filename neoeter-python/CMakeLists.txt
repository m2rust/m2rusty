cmake_minimum_required(VERSION 3.20)
project(NeoeterPython VERSION 0.1.0 LANGUAGES C CXX)

# Python 2.7 host + bindings. One header root, one directory per package.

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(NEOETER_PYTHON_INCLUDE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/include")
include_directories("${NEOETER_PYTHON_INCLUDE_DIR}")

option(NEOETER_PYTHON_STANDALONE "Build Neoeter Python on its own" OFF)

if(NEOETER_PYTHON_STANDALONE)
    include(FetchContent)
    FetchContent_Declare(neoeter
        GIT_REPOSITORY https://github.com/open-mt2/neoeter.git
        GIT_TAG main)
    set(BUILD_TESTING_SAVED ${BUILD_TESTING})
    set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(neoeter)
    set(BUILD_TESTING ${BUILD_TESTING_SAVED} CACHE BOOL "" FORCE)
endif()

foreach(_package IN ITEMS host bindings)
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/src/${_package}/CMakeLists.txt")
        add_subdirectory("src/${_package}" "${CMAKE_BINARY_DIR}/src/${_package}")
    else()
        message(STATUS "Neoeter Python: src/${_package} not imported yet")
    endif()
endforeach()
