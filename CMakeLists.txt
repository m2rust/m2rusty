cmake_minimum_required(VERSION 3.20)
project(M2Rusty VERSION 0.1.0 LANGUAGES C CXX)

# Monorepo build: engine, Python host/bindings and game client in one tree.
# Each subproject keeps its own standalone option for building it alone; here
# they are added in dependency order instead of fetched from GitHub.

set(CMAKE_CXX_STANDARD 26)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# compile_commands.json in the build tree, for clangd and clang-tidy -p.
set(CMAKE_EXPORT_COMPILE_COMMANDS ON CACHE BOOL "" FORCE)

# Jumbo translation units for faster full builds. Set before the subprojects
# so every target picks it up; turn off to compile file by file.
option(M2_UNITY_BUILD "Build every target as unity (jumbo) sources" OFF)
set(CMAKE_UNITY_BUILD ${M2_UNITY_BUILD})
set(CMAKE_UNITY_BUILD_BATCH_SIZE 16 CACHE STRING "Sources per unity translation unit")

set(NEOETER_STANDALONE OFF CACHE BOOL "" FORCE)
set(NEOETER_PYTHON_STANDALONE OFF CACHE BOOL "" FORCE)
set(CLIENT_STANDALONE OFF CACHE BOOL "" FORCE)

add_subdirectory(neoeter)
add_subdirectory(neoeter-python)
add_subdirectory(client)
