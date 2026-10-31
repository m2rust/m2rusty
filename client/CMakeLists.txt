cmake_minimum_required(VERSION 3.20)
project(Client VERSION 0.1.0 LANGUAGES C CXX)

# Game layer: GameLib + UserInterface + CWebBrowser. One header root, one
# directory per package. Depends on neoeter and neoeter-python.

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(CLIENT_INCLUDE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/include")
include_directories("${CLIENT_INCLUDE_DIR}")

option(CLIENT_STANDALONE "Build the client on its own" OFF)

if(CLIENT_STANDALONE)
    include(FetchContent)
    foreach(_repo IN ITEMS neoeter neoeter-python)
        FetchContent_Declare(${_repo}
            GIT_REPOSITORY https://github.com/open-mt2/${_repo}.git
            GIT_TAG main)
    endforeach()
    set(BUILD_TESTING_SAVED ${BUILD_TESTING})
    set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(neoeter neoeter-python)
    set(BUILD_TESTING ${BUILD_TESTING_SAVED} CACHE BOOL "" FORCE)
endif()

# The browser stub has no dependencies; the UI links both.
foreach(_package IN ITEMS webbrowser game ui)
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/src/${_package}/CMakeLists.txt")
        add_subdirectory("src/${_package}" "${CMAKE_BINARY_DIR}/src/${_package}")
    else()
        message(STATUS "Client: src/${_package} not imported yet")
    endif()
endforeach()
